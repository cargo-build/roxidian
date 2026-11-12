//! Behavioral coverage of invocation roots, readonly documents and lazy browsing.

use crate::{
    cli::Launch,
    document::{self, Destination, Document, Node},
    theme::Theme,
    workspace,
};
use anyhow::Result;
use std::{collections::HashSet, fs};

#[test]
fn standalone_files_use_filename_grammars_without_changing_source() -> Result<()> {
    let directory = tempfile::tempdir()?;
    for (name, source) in [
        ("main.rs", "fn main() { println!(\"<script>\"); }\n"),
        ("Cargo.toml", "[package]\nname = \"rox\"\n"),
        ("flake.nix", "{ pkgs }: { enabled = true; }\n"),
        ("install.sh", "pacman -S ssh curl\n"),
        ("flake.lock", "{\"version\": 7}\n"),
    ] {
        let path = directory.path().join(name);
        fs::write(&path, source)?;
        let document = Document::load(&path)?;
        assert_eq!(document.source, source, "{name}");
        let html = document
            .source_syntax
            .expect("highlighted source")
            .render(source);
        assert!(html.contains("class=\"syn-"), "{name}: {html}");
        assert!(!html.contains("<script>"), "{name}: {html}");
    }
    let path = directory.path().join("unknown.unsupported");
    fs::write(&path, "<script>&</script>")?;
    let document = Document::load(&path)?;
    assert!(document.source_syntax.is_none());
    assert_eq!(document.source, "<script>&</script>");
    Ok(())
}

#[test]
fn soft_wrapped_markdown_flows_while_explicit_breaks_remain() {
    let blocks = document::parse("one soft\nwrapped paragraph  \nhard break");
    assert_eq!(
        document::plain(&blocks),
        "one soft wrapped paragraph\nhard break"
    );
}

#[test]
fn fenced_highlighting_keeps_exact_text_and_inert_html() {
    let source = "pacman -S ssh pacman curl\nprintf '<script>alert(1)</script>'\n";
    let blocks = document::parse(&format!("```bash\n{source}```\n"));
    assert_eq!(document::plain(&blocks), source);
    let Node::Element(_, children) = &blocks[0] else {
        panic!("code block")
    };
    let Node::Syntax { html, .. } = &children[0] else {
        panic!("highlighted code")
    };
    assert!(html.contains("<span class=\"syn-command\">pacman</span>"));
    assert!(html.contains("<span class=\"syn-argument\">pacman</span>"));
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
}

#[test]
fn file_arguments_keep_terminal_workspace_even_outside_it() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let cwd = temp.path().join("nix-config");
    fs::create_dir(&cwd)?;
    let file = temp.path().join("outside.md");
    fs::write(&file, "# Outside")?;
    let absolute = Launch::resolve(&cwd, Some(&file))?;
    let relative = Launch::resolve(&cwd, Some(std::path::Path::new("../outside.md")))?;
    assert_eq!(absolute, relative);
    assert_eq!(absolute.root, cwd.canonicalize()?);
    assert_eq!(absolute.file, Some(file.canonicalize()?));
    Ok(())
}

#[test]
fn directories_dot_and_no_argument_select_the_expected_root() -> Result<()> {
    let temp = tempfile::tempdir()?;
    fs::create_dir(temp.path().join("doc"))?;
    [None, Some(std::path::Path::new("."))]
        .into_iter()
        .try_for_each(|argument| -> Result<()> {
            assert_eq!(
                Launch::resolve(temp.path(), argument)?.root,
                temp.path().canonicalize()?
            );
            Ok(())
        })?;
    [temp.path().join("doc"), std::path::PathBuf::from("./doc/")]
        .iter()
        .try_for_each(|path| -> Result<()> {
            let launch = Launch::resolve(temp.path(), Some(path))?;
            assert_eq!(launch.root, temp.path().join("doc").canonicalize()?);
            assert_eq!(launch.file, None);
            Ok(())
        })?;
    assert!(Launch::resolve(temp.path(), Some(std::path::Path::new("missing"))).is_err());
    Ok(())
}

#[test]
fn directory_tree_is_lazy_sorted_and_includes_source_files() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let folder = temp.path().join("notes");
    fs::create_dir(&folder)?;
    fs::create_dir(temp.path().join(".git"))?;
    fs::write(folder.join("child.md"), "child")?;
    fs::write(temp.path().join("B.MD"), "b")?;
    fs::write(temp.path().join("a.markdown"), "a")?;
    fs::write(temp.path().join("unrelated.rs"), "fn main() {}")?;
    let rows = workspace::read(temp.path(), &HashSet::new())?;
    assert_eq!(
        rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["notes", "a.markdown", "B.MD", "unrelated.rs"]
    );
    let expanded = HashSet::from([folder]);
    let rows = workspace::read(temp.path(), &expanded)?;
    assert_eq!(rows[1].name, "child.md");
    assert_eq!(rows[1].depth, 1);
    assert!(workspace::read(&temp.path().join("missing"), &expanded).is_err());
    Ok(())
}

#[test]
fn workspace_fuzzy_search_includes_configs_and_sources_but_skips_binary() -> Result<()> {
    let temp = tempfile::tempdir()?;
    fs::write(
        temp.path().join("settings.toml"),
        "endpoint = 'production'\n",
    )?;
    fs::write(temp.path().join("main.rs"), "fn configure_endpoint() {}\n")?;
    fs::write(temp.path().join("notes.md"), "# Endpoint guide\n")?;
    fs::write(temp.path().join("binary.dat"), b"endpoint\0binary")?;
    fs::create_dir(temp.path().join("target"))?;
    fs::write(temp.path().join("target/ignored.txt"), "endpoint")?;
    let results = crate::search::workspace(temp.path(), "endpnt");
    let names = results
        .hits
        .iter()
        .map(|hit| {
            hit.path
                .file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<HashSet<_>>();
    assert_eq!(
        names,
        HashSet::from(["settings.toml".into(), "main.rs".into(), "notes.md".into()])
    );
    assert!(results.hits.iter().all(|hit| hit.line == Some(1)));
    assert!(crate::search::workspace(temp.path(), "").hits.is_empty());
    Ok(())
}

#[test]
fn find_is_literal_case_insensitive_and_preserves_unicode_boundaries() {
    let text = "Привет 🦀 ПРИВЕТ [x]";
    let ranges = crate::search::occurrences(text, "привет");
    assert_eq!(
        ranges
            .iter()
            .map(|range| &text[range.clone()])
            .collect::<Vec<_>>(),
        ["Привет", "ПРИВЕТ"]
    );
    assert_eq!(crate::search::occurrences(text, "[x]").len(), 1);
    assert!(crate::search::occurrences(text, "").is_empty());
}

#[test]
fn fuzzy_search_checks_past_the_start_of_long_config_lines() -> Result<()> {
    let temp = tempfile::tempdir()?;
    fs::write(
        temp.path().join("settings.json"),
        format!("{} endpoint", "x".repeat(2000)),
    )?;
    let result = crate::search::workspace(temp.path(), "endpoint");
    assert_eq!(result.hits.len(), 1);
    assert!(result.hits[0].excerpt.contains("endpoint"));
    Ok(())
}

#[test]
fn load_preserves_exact_source_and_never_writes() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("unicode.md");
    let source = "# Привет 🦀\r\n\r\n**bold**\t`code`\n";
    fs::write(&path, source)?;
    let before = fs::read(&path)?;
    let document = Document::load(&path)?;
    assert_eq!(&*document.source, source);
    assert_eq!(document.lines, 3);
    assert_eq!(fs::read(&path)?, before);
    fs::write(&path, "# Changed")?;
    assert_eq!(&*Document::load(&path)?.source, "# Changed");
    assert_eq!(&*document.source, source);
    Ok(())
}

#[test]
fn invalid_utf8_binary_missing_and_oversize_are_errors() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("invalid.md");
    assert!(Document::load(&path).is_err());
    fs::write(&path, [0xff, 0xfe])?;
    assert!(Document::load(&path).is_err());
    fs::write(&path, "text\0binary")?;
    assert!(Document::load(&path).is_err());
    fs::File::create(&path)?.set_len(document::MAX_BYTES + 1)?;
    assert!(Document::load(&path).is_err());
    assert!(Document::load(temp.path()).is_err());
    Ok(())
}

#[test]
fn commonmark_and_gfm_blocks_keep_their_semantics() {
    let source = "# Heading\n\n> A **bold** quote\n\n- [x] complete\n- [ ] pending\n\n| A | B |\n|---|---|\n| one | two |\n\n```rust\nfn main() {}\n```\n\n<script>alert('inert')</script>\n";
    let blocks = document::parse(source);
    assert!(matches!(
        &blocks[0],
        Node::Element(pulldown_cmark::Tag::Heading { .. }, _)
    ));
    assert!(
        blocks
            .iter()
            .any(|node| matches!(node, Node::Element(pulldown_cmark::Tag::BlockQuote(_), _)))
    );
    assert!(
        blocks
            .iter()
            .any(|node| matches!(node, Node::Element(pulldown_cmark::Tag::Table(_), _)))
    );
    let code = blocks.iter().find_map(|node| match node {
        Node::Element(pulldown_cmark::Tag::CodeBlock(_), children) => {
            Some(document::plain(children))
        }
        _ => None,
    });
    assert_eq!(code.as_deref(), Some("fn main() {}\n"));
    assert!(document::plain(&blocks).contains("<script>alert('inert')</script>"));
    assert!(document::plain(&blocks).contains("☑ complete"));
}

#[test]
fn local_links_decode_spaces_and_resolve_from_document_not_workspace() -> Result<()> {
    let path = std::path::Path::new("/tmp/example/doc/readme.md");
    assert_eq!(
        document::destination(path, "../other%20file.md")?,
        Destination::Local("/tmp/example/other file.md".into())
    );
    assert_eq!(
        document::destination(path, "#heading")?,
        Destination::Anchor("heading".into())
    );
    assert!(matches!(
        document::destination(path, "https://example.org")?,
        Destination::External(_)
    ));
    assert!(document::destination(path, "javascript:alert(1)").is_err());
    Ok(())
}

#[test]
fn dark_is_default_and_themes_roundtrip() {
    assert_eq!(Theme::default(), Theme::Synthwave);
    assert_eq!(Theme::Light.toggle(), Theme::Synthwave);
    assert_eq!(Theme::Synthwave.toggle(), Theme::Light);
}

#[test]
fn native_hit_test_after_removed_content() {
    use blitz_dom::Document as _;
    use dioxus::prelude::*;
    use std::{cell::Cell, rc::Rc};
    fn changing() -> Element {
        let expanded = use_context::<Rc<Cell<bool>>>();
        rsx! { style { {"div { width: 800px; height: 600px; } p { height: 100px; }"} }
            div { if expanded.get() { p { "removed content" } } p { "surviving content" } }
        }
    }
    let expanded = Rc::new(Cell::new(true));
    let vdom = VirtualDom::new(changing);
    vdom.provide_root_context(expanded.clone());
    let mut doc = dioxus_native::DioxusDocument::new(
        vdom,
        dioxus_native::DocumentConfig {
            viewport: Some(blitz_traits::shell::Viewport::new(
                800,
                600,
                1.,
                blitz_traits::shell::ColorScheme::Dark,
            )),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.inner.borrow_mut().resolve(0.);
    expanded.set(false);
    doc.vdom.mark_dirty(ScopeId::APP);
    doc.poll(None);
    let _ = doc.inner.borrow().hit(10., 10.);
}

#[test]
fn wayland_clipboard_roundtrip_restores_selection() {
    use std::io::Write as _;
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("skip: no Wayland session");
        return;
    }
    let paste = || std::process::Command::new("wl-paste").arg("-n").output();
    if paste().is_err() {
        eprintln!("skip: wl-paste unavailable");
        return;
    }
    let before = paste()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| out.stdout);
    let probe = "roxidian clipboard probe 42";
    arboard::Clipboard::new()
        .expect("clipboard")
        .set_text(probe)
        .expect("clipboard write");
    let after = paste().expect("wl-paste").stdout;
    assert_eq!(String::from_utf8_lossy(&after), probe);
    match before {
        Some(bytes) => {
            let mut child = std::process::Command::new("wl-copy")
                .arg("-n")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("wl-copy");
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(&bytes)
                .expect("restore");
            child.wait().expect("wl-copy wait");
        }
        None => {
            std::process::Command::new("wl-copy")
                .arg("--clear")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .expect("wl-copy clear");
        }
    }
}
