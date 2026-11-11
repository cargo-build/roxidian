//! Semantic HTML highlighting for fenced code blocks.
//!
//! Shell fences are read as a `tree-sitter-bash` syntax tree, so a word is
//! classified by syntactic position: the command name differs from a positional
//! argument, and only a flag known to take a value promotes the next argument to
//! a value. Every other fence uses `syntect` scopes from the `two_face` set.
//!
//! Output is a sequence of `<span class="syn-…">` elements with escaped text and
//! no wrapper element, so fenced content never creates markup, attributes or
//! script of its own.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow};
use syntect::parsing::{ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet};
use tree_sitter::{Node, Parser};

/// Opens a fence with the shell walker instead of a `syntect` grammar.
const SHELL_TOKENS: &[&str] = &[
    "ash",
    "bash",
    "dash",
    "ksh",
    "mksh",
    "posh",
    "sh",
    "shell",
    "shellscript",
    "zsh",
];

/// Commands whose heredoc body is a script rather than data.
const SHELL_INTERPRETERS: &[&str] = &[
    "ash", "bash", "busybox", "dash", "ksh", "mksh", "sh", "ssh", "zsh",
];

/// Heredoc nesting walked as script; deeper bodies stay strings.
const MAX_SCRIPT_NESTING: usize = 3;

/// Syntax-tree depth that still receives spans; deeper nodes stay plain text.
const MAX_TREE_DEPTH: usize = 128;

/// Reserved words highlighted as [`Class::Keyword`].
const SHELL_KEYWORDS: &[&str] = &[
    "case", "coproc", "declare", "do", "done", "elif", "else", "esac", "export", "fi", "for",
    "function", "if", "in", "local", "readonly", "select", "then", "time", "typeset", "unset",
    "unsetenv", "until", "while",
];

/// Child kinds walked separately while their parent keeps the literal class.
const EXPANSIONS: &[&str] = &[
    "arithmetic_expansion",
    "command_substitution",
    "expansion",
    "process_substitution",
    "simple_expansion",
];

/// [`EXPANSIONS`] plus nested quoted strings.
const EXPANSIONS_AND_STRINGS: &[&str] = &[
    "ansi_c_string",
    "arithmetic_expansion",
    "command_substitution",
    "expansion",
    "process_substitution",
    "raw_string",
    "simple_expansion",
    "string",
    "translated_string",
];

/// Nested commands inside a parameter expansion keep their own styling.
const COMMAND_SUBSTITUTIONS: &[&str] = &["command_substitution", "process_substitution"];

/// Flags whose value is a separate argument, per command.
///
/// Shell syntax alone cannot tell `grep -A 3 pattern` from `pacman -S package`,
/// so values are only propagated for the flags listed here. The table is
/// deliberately bounded and incomplete: an unknown command never promotes a
/// following argument to a value, and an unlisted flag of a known command leaves
/// its neighbour a positional argument.
const VALUE_FLAGS: &[(&str, &[&str])] = &[
    (
        "awk",
        &["-F", "-f", "-v", "--field-separator", "--file", "--assign"],
    ),
    (
        "cargo",
        &[
            "--bin",
            "--config",
            "--example",
            "--features",
            "--jobs",
            "--manifest-path",
            "--package",
            "--profile",
            "--target",
            "--target-dir",
            "--test",
            "-Z",
            "-j",
            "-p",
        ],
    ),
    (
        "curl",
        &[
            "--cacert",
            "--cert",
            "--connect-timeout",
            "--cookie",
            "--cookie-jar",
            "--data",
            "--data-binary",
            "--data-raw",
            "--data-urlencode",
            "--dump-header",
            "--form",
            "--form-string",
            "--header",
            "--key",
            "--max-time",
            "--output",
            "--proxy",
            "--range",
            "--referer",
            "--request",
            "--retry",
            "--retry-delay",
            "--url",
            "--user",
            "--user-agent",
            "--write-out",
            "-A",
            "-D",
            "-F",
            "-H",
            "-K",
            "-T",
            "-X",
            "-b",
            "-c",
            "-d",
            "-e",
            "-m",
            "-o",
            "-r",
            "-u",
            "-w",
            "-x",
        ],
    ),
    (
        "cut",
        &[
            "--bytes",
            "--characters",
            "--delimiter",
            "--fields",
            "--output-delimiter",
            "-b",
            "-c",
            "-d",
            "-f",
        ],
    ),
    (
        "docker",
        &[
            "--config",
            "--context",
            "--host",
            "--log-level",
            "--tlscacert",
            "--tlscert",
            "--tlskey",
            "-H",
            "-c",
            "-l",
        ],
    ),
    (
        "find",
        &[
            "-group",
            "-iname",
            "-ipath",
            "-maxdepth",
            "-mindepth",
            "-mmin",
            "-mtime",
            "-name",
            "-newer",
            "-path",
            "-perm",
            "-printf",
            "-size",
            "-type",
            "-user",
        ],
    ),
    (
        "git",
        &[
            "--config-env",
            "--exec-path",
            "--git-dir",
            "--namespace",
            "--work-tree",
            "-C",
            "-c",
        ],
    ),
    (
        "grep",
        &[
            "--after-context",
            "--before-context",
            "--context",
            "--devices",
            "--directories",
            "--exclude",
            "--exclude-dir",
            "--file",
            "--include",
            "--label",
            "--max-count",
            "--regexp",
            "-A",
            "-B",
            "-C",
            "-D",
            "-d",
            "-e",
            "-f",
            "-m",
        ],
    ),
    ("head", &["--bytes", "--lines", "-c", "-n"]),
    (
        "journalctl",
        &[
            "--after-cursor",
            "--boot",
            "--cursor",
            "--directory",
            "--file",
            "--identifier",
            "--lines",
            "--output",
            "--priority",
            "--since",
            "--unit",
            "--until",
            "-D",
            "-M",
            "-n",
            "-o",
            "-p",
            "-t",
            "-u",
        ],
    ),
    (
        "kubectl",
        &[
            "--as",
            "--cluster",
            "--context",
            "--field-selector",
            "--filename",
            "--kubeconfig",
            "--namespace",
            "--output",
            "--selector",
            "--server",
            "--token",
            "--user",
            "-f",
            "-l",
            "-n",
            "-o",
        ],
    ),
    (
        "nix",
        &[
            "--arg",
            "--argstr",
            "--attr",
            "--expr",
            "--file",
            "--include",
            "--option",
            "--store",
            "-A",
            "-I",
            "-f",
        ],
    ),
    (
        "pacman",
        &[
            "--arch",
            "--assume-installed",
            "--cachedir",
            "--config",
            "--dbpath",
            "--gpgdir",
            "--hookdir",
            "--ignore",
            "--ignoregroup",
            "--logfile",
            "--overwrite",
            "--print-format",
            "--root",
            "--sysroot",
            "-b",
            "-r",
        ],
    ),
    (
        "python",
        &["--check-hash-based-pycs", "-W", "-X", "-c", "-m"],
    ),
    (
        "rg",
        &[
            "--after-context",
            "--before-context",
            "--context",
            "--encoding",
            "--glob",
            "--max-count",
            "--max-depth",
            "--path-separator",
            "--pre",
            "--replace",
            "--threads",
            "--type",
            "--type-not",
            "-A",
            "-B",
            "-C",
            "-e",
            "-f",
            "-g",
            "-m",
            "-t",
            "-T",
        ],
    ),
    ("scp", &["-F", "-J", "-P", "-S", "-c", "-i", "-l", "-o"]),
    (
        "sed",
        &[
            "--expression",
            "--file",
            "--in-place",
            "--line-length",
            "--suffix",
            "-e",
            "-f",
            "-i",
            "-l",
        ],
    ),
    (
        "sort",
        &[
            "--buffer-size",
            "--field-separator",
            "--key",
            "--output",
            "--sort",
            "--temporary-directory",
            "-S",
            "-T",
            "-k",
            "-o",
            "-t",
        ],
    ),
    (
        "ssh",
        &[
            "--escape-char",
            "--identity",
            "--jump-host",
            "--login-name",
            "--port",
            "-B",
            "-D",
            "-E",
            "-F",
            "-I",
            "-J",
            "-L",
            "-O",
            "-Q",
            "-R",
            "-S",
            "-W",
            "-b",
            "-c",
            "-e",
            "-i",
            "-l",
            "-m",
            "-o",
            "-p",
            "-w",
        ],
    ),
    (
        "systemctl",
        &[
            "--host",
            "--kill-whom",
            "--lines",
            "--machine",
            "--output",
            "--property",
            "--signal",
            "--state",
            "--type",
            "-H",
            "-M",
            "-n",
            "-o",
            "-p",
            "-t",
        ],
    ),
    (
        "tail",
        &[
            "--bytes",
            "--lines",
            "--pid",
            "--sleep-interval",
            "-c",
            "-n",
        ],
    ),
    (
        "tar",
        &[
            "--directory",
            "--exclude",
            "--file",
            "--group",
            "--mode",
            "--owner",
            "--transform",
            "-C",
            "-f",
        ],
    ),
    (
        "wget",
        &[
            "--directory-prefix",
            "--header",
            "--limit-rate",
            "--output-document",
            "--password",
            "--post-data",
            "--timeout",
            "--tries",
            "--user",
            "--user-agent",
            "-O",
            "-P",
            "-T",
            "-U",
            "-e",
            "-t",
        ],
    ),
];

/// CSS classes a `syntect` scope may produce.
///
/// The first atom of the innermost scope selects the class, so every generic
/// class stays in the same `syn-` namespace as the shell classes.
const SCOPE_CLASSES: &[(&str, Class)] = &[
    ("constant.numeric", Class::Number),
    ("keyword", Class::Keyword),
    ("string", Class::String),
    ("comment", Class::Comment),
    ("constant", Class::Constant),
    ("storage", Class::Storage),
    ("entity", Class::Entity),
    ("support", Class::Support),
    ("variable", Class::Variable),
    ("punctuation", Class::Punctuation),
    ("invalid", Class::Invalid),
    ("meta", Class::Meta),
];

/// Non-standard fence tokens that name a different `syntect` language.
const LANGUAGE_ALIASES: &[(&str, &str)] = &[
    ("c#", "csharp"),
    ("c++", "cpp"),
    ("f#", "fsharp"),
    ("htm", "html"),
    ("jsx", "javascript"),
    ("mjs", "javascript"),
    ("tsx", "typescript"),
];

/// CSS class of one highlighted token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// Command name: bold and coloured.
    Command,
    /// Positional argument: normal weight.
    Argument,
    /// Option token such as `-S`, `--needed` or `-abc`.
    Flag,
    /// Variable name or expansion such as `$HOME`.
    Env,
    /// Value of a recognised flag: italic.
    Value,
    /// Value of an `ENV=value` assignment: italic.
    EnvValue,
    /// Quoted string, heredoc delimiter or heredoc data.
    String,
    /// Comment.
    Comment,
    /// Shell keyword.
    Keyword,
    /// Numeric literal.
    Number,
    /// Generic `syntect` classes below this point.
    Constant,
    Entity,
    Support,
    Storage,
    Variable,
    Punctuation,
    Invalid,
    Meta,
}

impl Class {
    /// JSON-free CSS class name, including the `syn-` prefix.
    const fn name(self) -> &'static str {
        match self {
            Class::Command => "syn-command",
            Class::Argument => "syn-argument",
            Class::Flag => "syn-flag",
            Class::Env => "syn-env",
            Class::Value => "syn-value",
            Class::EnvValue => "syn-env-value",
            Class::String => "syn-string",
            Class::Comment => "syn-comment",
            Class::Keyword => "syn-keyword",
            Class::Number => "syn-number",
            Class::Constant => "syn-constant",
            Class::Entity => "syn-entity",
            Class::Support => "syn-support",
            Class::Storage => "syn-storage",
            Class::Variable => "syn-variable",
            Class::Punctuation => "syn-punctuation",
            Class::Invalid => "syn-invalid",
            Class::Meta => "syn-meta",
        }
    }
}

/// Highlights one fenced block as escaped HTML spans.
///
/// `language` is the fence info string; surrounding braces, a dot prefix, case
/// and trailing info words are ignored. A shell fence is walked as a
/// `tree-sitter-bash` tree, any other fence uses a `syntect` grammar, and an
/// unknown language returns escaped plain text.
///
/// # Errors
///
/// Returns an error when the shell grammar cannot be loaded, the shell parser
/// yields no tree, or a `syntect` grammar fails on the source. Malformed source
/// text is not an error: it is highlighted as far as the parsers recover.
pub fn highlight(source: &str, language: &str) -> Result<String> {
    let token = language_token(language);
    let markup = match SHELL_TOKENS.contains(&token.as_str()) {
        true => highlight_shell(source),
        false => highlight_general(source, &token),
    }?;
    Ok(markup.render(source))
}

/// Select a grammar using the filename and extension of a standalone file. @req DECL0001
pub fn highlight_file(source: &str, path: &std::path::Path) -> Result<Option<Markup>> {
    let language = match path.file_name().and_then(|name| name.to_str()) {
        Some("flake.lock") => "json",
        Some("Cargo.lock") => "toml",
        Some(".env" | ".envrc") => "bash",
        Some("justfile" | "Justfile" | "Makefile") => "makefile",
        Some("Dockerfile") => "dockerfile",
        _ => {
            let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
                return Ok(None);
            };
            extension
        }
    };
    let token = language_token(language);
    if !SHELL_TOKENS.contains(&token.as_str()) && lookup_syntax(syntax_set(), &token).is_none() {
        return Ok(None);
    }
    match SHELL_TOKENS.contains(&token.as_str()) {
        true => highlight_shell(source),
        false => highlight_general(source, &token),
    }
    .map(Some)
}

/// Reduces a fence info string to its lowercase language token.
fn language_token(language: &str) -> String {
    let trimmed = language.trim().trim_start_matches(['{', '.']);
    let first = trimmed
        .split(|character: char| character.is_whitespace() || matches!(character, ',' | ';'))
        .next()
        .unwrap_or_default();
    first
        .trim_start_matches(['{', '.'])
        .trim_end_matches('}')
        .to_ascii_lowercase()
}

/// Ordered, non-overlapping HTML segments over one source text.
#[derive(Debug, Default)]
pub struct Markup {
    segments: Vec<Segment>,
}

/// One styled byte range of the highlighted source.
#[derive(Debug)]
struct Segment {
    range: Range<usize>,
    class: Class,
}

impl Markup {
    pub fn spans<'a>(
        &'a self,
        source: &'a str,
    ) -> impl Iterator<Item = (Range<usize>, &'static str)> + 'a {
        self.segments.iter().map(move |segment| {
            (
                floor_char_boundary(source, segment.range.start)
                    ..floor_char_boundary(source, segment.range.end),
                segment.class.name(),
            )
        })
    }
    /// Marks `range` with `class`, merging with an adjacent identical segment.
    fn mark(&mut self, range: Range<usize>, class: Class) {
        if range.start >= range.end {
            return;
        }
        if let Some(last) = self.segments.last_mut()
            && last.class == class
            && last.range.end == range.start
        {
            last.range.end = range.end;
            return;
        }
        self.segments.push(Segment { range, class });
    }

    /// Renders the segments, escaping every byte of `source` exactly once.
    pub fn render(&self, source: &str) -> String {
        let mut out = String::with_capacity(source.len() + self.segments.len() * 32);
        let mut cursor = 0;
        for segment in &self.segments {
            let start = floor_char_boundary(source, segment.range.start).max(cursor);
            let end = floor_char_boundary(source, segment.range.end);
            if start >= end {
                continue;
            }
            escape_into(&mut out, &source[cursor..start]);
            out.push_str("<span class=\"");
            out.push_str(segment.class.name());
            out.push_str("\">");
            escape_into(&mut out, &source[start..end]);
            out.push_str("</span>");
            cursor = end;
        }
        escape_into(&mut out, &source[cursor..]);
        out
    }
}

/// Escapes the characters that could end the text context or start markup.
fn escape_into(out: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
}

/// Largest index at or below `index` that splits `text` between characters.
fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Slices `source` at `range`, clamping to character boundaries.
fn slice(source: &str, range: Range<usize>) -> &str {
    let start = floor_char_boundary(source, range.start);
    let end = floor_char_boundary(source, range.end).max(start);
    &source[start..end]
}

/// Child nodes of `node` collected for field-indexed iteration.
fn node_children<'tree>(node: Node<'tree>) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

/// Byte range of `node` inside its own source text.
fn node_range(node: Node<'_>) -> Range<usize> {
    node.start_byte()..node.end_byte()
}

/// Highlights a shell fence through the bash grammar.
fn highlight_shell(source: &str) -> Result<Markup> {
    let mut parser = bash_parser()?;
    let mut markup = Markup::default();
    shell_into(source, 0, 0, &mut parser, &mut markup)?;
    Ok(markup)
}

/// Builds the bash parser once per shell fence, including nested heredocs.
fn bash_parser() -> Result<Parser> {
    let mut parser = Parser::new();
    let language = tree_sitter::Language::from(tree_sitter_bash::LANGUAGE);
    parser
        .set_language(&language)
        .context("loading the bash grammar for shell highlighting")?;
    Ok(parser)
}

/// Adds the spans of `source` to `markup`, shifted by `base` bytes.
fn shell_into(
    source: &str,
    base: usize,
    nesting: usize,
    parser: &mut Parser,
    markup: &mut Markup,
) -> Result<()> {
    let tree = parser.parse(source, None).ok_or_else(|| {
        anyhow!("bash parser produced no syntax tree at heredoc nesting level {nesting}")
    })?;
    ShellHighlighter {
        source,
        base,
        nesting,
        markup,
    }
    .walk(tree.root_node(), 0)
}

/// Flags detected for the arguments of one command.
#[derive(Default)]
struct FlagState {
    pending_value: bool,
    end_of_flags: bool,
}

/// Position-aware walker over one shell syntax tree.
struct ShellHighlighter<'source, 'markup> {
    source: &'source str,
    base: usize,
    nesting: usize,
    markup: &'markup mut Markup,
}

impl<'source, 'markup> ShellHighlighter<'source, 'markup> {
    /// Text of `node` inside this walker's source slice.
    fn text(&self, node: Node<'_>) -> &'source str {
        slice(self.source, node_range(node))
    }

    /// Absolute range of `node` inside the rendered root source.
    fn range(&self, node: Node<'_>) -> Range<usize> {
        self.base + node.start_byte()..self.base + node.end_byte()
    }

    /// Marks `node` with `class`.
    fn mark_node(&mut self, node: Node<'_>, class: Class) {
        let range = self.range(node);
        self.markup.mark(range, class);
    }

    /// Marks `node` with `class`, walking `excluded` children separately.
    fn walk_around(
        &mut self,
        node: Node<'_>,
        class: Class,
        excluded: &[&str],
        depth: usize,
    ) -> Result<()> {
        let mut position = node.start_byte();
        for child in node_children(node) {
            if !excluded.contains(&child.kind()) {
                continue;
            }
            self.markup
                .mark(self.base + position..self.base + child.start_byte(), class);
            self.walk(child, depth + 1)?;
            position = child.end_byte();
        }
        self.markup
            .mark(self.base + position..self.base + node.end_byte(), class);
        Ok(())
    }

    /// Walks named children and marks anonymous reserved words.
    fn walk_children(&mut self, node: Node<'_>, depth: usize) -> Result<()> {
        for child in node_children(node) {
            match child.is_named() {
                true => self.walk(child, depth + 1)?,
                false if SHELL_KEYWORDS.contains(&child.kind()) => {
                    self.mark_node(child, Class::Keyword);
                }
                false => {}
            }
        }
        Ok(())
    }

    /// Walks one node by kind.
    fn walk(&mut self, node: Node<'_>, depth: usize) -> Result<()> {
        if depth > MAX_TREE_DEPTH {
            return Ok(());
        }
        match node.kind() {
            "comment" => self.mark_node(node, Class::Comment),
            "command" => return self.walk_command(node, depth),
            "command_name" => self.mark_node(node, Class::Command),
            "function_definition" => {
                if let Some(name) = node.child_by_field_name("name") {
                    self.mark_node(name, Class::Command);
                }
                return self.walk_children(node, depth);
            }
            "variable_assignment" => return self.walk_assignment(node, depth),
            "variable_name" | "special_variable_name" | "simple_expansion" => {
                self.mark_node(node, Class::Env);
            }
            "expansion" => {
                return self.walk_around(node, Class::Env, COMMAND_SUBSTITUTIONS, depth);
            }
            "arithmetic_expansion" => self.mark_node(node, Class::Env),
            "command_substitution" | "process_substitution" => {
                return self.walk_children(node, depth);
            }
            "string" | "raw_string" | "ansi_c_string" | "translated_string" => {
                return self.walk_around(node, Class::String, EXPANSIONS, depth);
            }
            "number" => self.mark_node(node, Class::Number),
            "test_operator" => self.mark_node(node, Class::Flag),
            "heredoc_redirect" => return self.walk_heredoc(node, depth),
            "heredoc_start" | "heredoc_end" => self.mark_node(node, Class::String),
            _ => return self.walk_children(node, depth),
        }
        Ok(())
    }

    /// Walks `command`: the name keeps its own class, arguments take roles.
    fn walk_command(&mut self, node: Node<'_>, depth: usize) -> Result<()> {
        let command = self.command_word(node);
        let mut flags = FlagState::default();
        for (index, child) in node_children(node).iter().enumerate() {
            match node.field_name_for_child(index as u32) {
                Some("name") => self.mark_node(*child, Class::Command),
                Some("argument") => {
                    self.walk_argument(*child, command.as_deref(), &mut flags, depth)?;
                }
                _ => self.walk(*child, depth + 1)?,
            }
        }
        Ok(())
    }

    /// Lowercase basename of a command node, used for flag knowledge.
    fn command_word(&self, node: Node<'_>) -> Option<String> {
        let name = node.child_by_field_name("name")?;
        let text = self.text(name);
        let base = text.rsplit('/').next().unwrap_or(text);
        Some(base.to_ascii_lowercase())
    }

    /// Walks one argument, promoting it to a value after a known value flag.
    fn walk_argument(
        &mut self,
        node: Node<'_>,
        command: Option<&str>,
        flags: &mut FlagState,
        depth: usize,
    ) -> Result<()> {
        if flags.pending_value {
            flags.pending_value = false;
            return self.walk_around(node, Class::Value, EXPANSIONS, depth);
        }
        match node.kind() {
            "word" => self.walk_word(node, command, flags),
            "number" => {
                self.mark_node(node, Class::Number);
                Ok(())
            }
            "arithmetic_expansion" => {
                self.mark_node(node, Class::Env);
                Ok(())
            }
            "concatenation" => self.walk_concatenation(node, depth),
            _ => self.walk(node, depth + 1),
        }
    }

    /// Classifies a literal argument token.
    fn walk_word(
        &mut self,
        node: Node<'_>,
        command: Option<&str>,
        flags: &mut FlagState,
    ) -> Result<()> {
        let text = self.text(node);
        if flags.end_of_flags {
            self.mark_node(node, Class::Argument);
            return Ok(());
        }
        if let Some(equal) = flag_value_separator(text) {
            let start = self.base + node.start_byte();
            self.markup.mark(start..start + equal, Class::Flag);
            self.markup
                .mark(start + equal + 1..start + text.len(), Class::Value);
            return Ok(());
        }
        match text {
            "--" => {
                flags.end_of_flags = true;
                self.mark_node(node, Class::Argument);
            }
            _ => match is_integer(text) {
                true => self.mark_node(node, Class::Number),
                false => match is_flag_text(text) {
                    true => {
                        self.mark_node(node, Class::Flag);
                        flags.pending_value = takes_value(command, text);
                    }
                    false => self.mark_node(node, Class::Argument),
                },
            },
        }
        Ok(())
    }

    /// Walks an argument glued from a word and quoted parts.
    fn walk_concatenation(&mut self, node: Node<'_>, depth: usize) -> Result<()> {
        let children = node_children(node);
        let Some(first) = children.first().copied() else {
            return Ok(());
        };
        let text = self.text(first);
        if !matches!(first.kind(), "word") || !text.starts_with('-') {
            return self.walk_around(node, Class::Argument, EXPANSIONS_AND_STRINGS, depth);
        }
        match flag_value_separator(text) {
            Some(equal) => {
                let start = self.base + first.start_byte();
                self.markup.mark(start..start + equal, Class::Flag);
                self.mark_value_tail(node, first.start_byte() + equal + 1, depth)
            }
            None => self.walk_around(node, Class::Argument, EXPANSIONS_AND_STRINGS, depth),
        }
    }

    /// Marks the remainder of a `--key=value` argument as a flag value.
    fn mark_value_tail(&mut self, node: Node<'_>, value_start: usize, depth: usize) -> Result<()> {
        let mut position = value_start;
        for child in node_children(node) {
            if child.start_byte() < value_start {
                continue;
            }
            self.markup.mark(
                self.base + position..self.base + child.start_byte(),
                Class::Value,
            );
            match EXPANSIONS.contains(&child.kind()) {
                true => self.walk(child, depth + 1)?,
                false => self.mark_node(child, Class::Value),
            }
            position = child.end_byte();
        }
        self.markup.mark(
            self.base + position..self.base + node.end_byte(),
            Class::Value,
        );
        Ok(())
    }

    /// Walks an `ENV=value` assignment.
    fn walk_assignment(&mut self, node: Node<'_>, depth: usize) -> Result<()> {
        if let Some(name) = node.child_by_field_name("name") {
            self.mark_node(name, Class::Env);
        }
        match node.child_by_field_name("value") {
            Some(value) => match value.kind() {
                "array" => self.walk_children(value, depth + 1),
                _ => self.walk_around(value, Class::EnvValue, EXPANSIONS, depth),
            },
            None => Ok(()),
        }
    }

    /// Walks a heredoc: delimiters are strings, script bodies are scripts.
    fn walk_heredoc(&mut self, node: Node<'_>, depth: usize) -> Result<()> {
        for child in node_children(node) {
            match child.kind() {
                "heredoc_start" | "heredoc_end" => self.mark_node(child, Class::String),
                "heredoc_body" => self.walk_heredoc_body(node, child, depth)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// Highlights a heredoc body as a nested script when its owner runs one.
    fn walk_heredoc_body(
        &mut self,
        redirect: Node<'_>,
        body: Node<'_>,
        depth: usize,
    ) -> Result<()> {
        match self.heredoc_is_script(redirect) && self.nesting < MAX_SCRIPT_NESTING {
            true => {
                let text = self.text(body);
                let mut parser = bash_parser()?;
                shell_into(
                    text,
                    self.base + body.start_byte(),
                    self.nesting + 1,
                    &mut parser,
                    self.markup,
                )
            }
            false => self.walk_around(body, Class::String, EXPANSIONS, depth),
        }
    }

    /// Whether the heredoc feeds a shell interpreter such as `ssh` or `bash`.
    fn heredoc_is_script(&self, redirect: Node<'_>) -> bool {
        let Some(parent) = redirect.parent() else {
            return false;
        };
        let owner = match parent.kind() {
            "redirected_statement" => parent.child_by_field_name("body"),
            "command" => Some(parent),
            _ => None,
        };
        let Some(owner) = owner else {
            return false;
        };
        if !matches!(owner.kind(), "command") {
            return false;
        }
        self.command_word(owner)
            .is_some_and(|command| SHELL_INTERPRETERS.contains(&command.as_str()))
    }
}

/// Index of the `=` in a `--key=value` token, if it has one.
fn flag_value_separator(text: &str) -> Option<usize> {
    if !text.starts_with('-') || text.len() < 2 {
        return None;
    }
    text.find('=').filter(|&index| index > 1)
}

/// Whether a literal token is an option rather than a positional argument.
fn is_flag_text(text: &str) -> bool {
    text.starts_with('-') && text.len() > 1
}

/// Whether a literal token is an integer, including a signed one.
fn is_integer(text: &str) -> bool {
    text.parse::<i64>().is_ok()
}

/// Whether `flag` of `command` consumes the following argument as its value.
fn takes_value(command: Option<&str>, flag: &str) -> bool {
    let Some(command) = command else {
        return false;
    };
    VALUE_FLAGS
        .iter()
        .find(|(name, _)| *name == command)
        .is_some_and(|(_, flags)| flags.contains(&flag))
}

/// Shared syntax set for every non-shell fence.
fn syntax_set() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(two_face::syntax::extra_newlines)
}

/// Highlights a non-shell fence, falling back to escaped plain text.
fn highlight_general(source: &str, token: &str) -> Result<Markup> {
    let syntaxes = syntax_set();
    let Some(syntax) = lookup_syntax(syntaxes, token) else {
        return Ok(Markup::default());
    };
    let mut parse_state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut markup = Markup::default();
    let mut classes: HashMap<Scope, Option<Class>> = HashMap::new();
    let mut offset = 0;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        let ops = parse_state.parse_line(line, syntaxes).with_context(|| {
            format!("parsing line {} of the `{}` fence", index + 1, syntax.name)
        })?;
        mark_line(
            &mut markup,
            &mut stack,
            &ops,
            line.len(),
            offset,
            &mut classes,
        )?;
        offset += line.len();
    }
    Ok(markup)
}

/// Finds the grammar for a fence token, following [`LANGUAGE_ALIASES`].
fn lookup_syntax<'set>(syntaxes: &'set SyntaxSet, token: &str) -> Option<&'set SyntaxReference> {
    let name = LANGUAGE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == token)
        .map(|(_, target)| *target)
        .unwrap_or(token);
    syntaxes.find_syntax_by_token(name)
}

/// Applies one line of scope operations and marks the styled ranges.
fn mark_line(
    markup: &mut Markup,
    stack: &mut ScopeStack,
    ops: &[(usize, ScopeStackOp)],
    line_len: usize,
    offset: usize,
    classes: &mut HashMap<Scope, Option<Class>>,
) -> Result<()> {
    let mut cursor = 0;
    for (position, op) in ops {
        let position = (*position).min(line_len);
        if position > cursor {
            mark_scope(markup, stack, offset + cursor..offset + position, classes);
        }
        stack.apply(op).context("applying a syntax scope change")?;
        cursor = position.max(cursor);
    }
    if cursor < line_len {
        mark_scope(markup, stack, offset + cursor..offset + line_len, classes);
    }
    Ok(())
}

/// Marks a token with the class of the innermost active scope.
fn mark_scope(
    markup: &mut Markup,
    stack: &ScopeStack,
    range: Range<usize>,
    classes: &mut HashMap<Scope, Option<Class>>,
) {
    let Some(scope) = stack.scopes.last().copied() else {
        return;
    };
    let class = *classes.entry(scope).or_insert_with(|| scope_class(scope));
    if let Some(class) = class {
        markup.mark(range, class);
    }
}

/// Maps a `syntect` scope to a [`Class`].
fn scope_class(scope: Scope) -> Option<Class> {
    let path = scope.build_string();
    SCOPE_CLASSES.iter().find_map(|&(prefix, class)| {
        let matches = path == prefix
            || (path.len() > prefix.len()
                && path.starts_with(prefix)
                && path.as_bytes()[prefix.len()] == b'.');
        matches.then_some(class)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SSH_SCRIPT: &str = "ssh root@host 'bash -s' <<'OUTER'\n\
set -e\n\
pacman -S package\n\
curl -fsS https://example.test | sh\n\
OUTER\n";

    /// Ordered `(class, text)` pairs of the emitted spans.
    fn spans(html: &str) -> Vec<(String, String)> {
        let mut spans = Vec::new();
        let mut rest = html;
        while let Some(open) = rest.find("<span class=\"") {
            let after = &rest[open + "<span class=\"".len()..];
            let Some(end_class) = after.find("\">") else {
                break;
            };
            let class = &after[..end_class];
            let content = &after[end_class + 2..];
            let Some(end_span) = content.find("</span>") else {
                break;
            };
            spans.push((class.to_owned(), unescape(&content[..end_span])));
            rest = &content[end_span + "</span>".len()..];
        }
        spans
    }

    /// Span classes in emission order.
    fn classes(html: &str) -> Vec<String> {
        spans(html).into_iter().map(|(class, _)| class).collect()
    }

    /// Visible text of the fragment after removing tags and entities.
    fn text_of(html: &str) -> String {
        let mut out = String::new();
        let mut rest = html;
        while let Some(open) = rest.find('<') {
            out.push_str(&rest[..open]);
            let Some(close) = rest[open..].find('>') else {
                break;
            };
            rest = &rest[open + close + 1..];
        }
        out.push_str(rest);
        unescape(&out)
    }

    /// Reverses [`escape_into`] for the four escaped characters.
    fn unescape(text: &str) -> String {
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&amp;", "&")
    }

    /// Highlights a fixture, reporting the error instead of panicking.
    fn highlighted(source: &str, language: &str) -> String {
        highlight(source, language).expect("highlighting a test fixture failed")
    }

    /// Highlights a fixture and checks that visible text matches the source.
    fn preserves(source: &str, language: &str) -> String {
        let html = highlighted(source, language);
        assert_eq!(
            text_of(&html),
            source,
            "visible text diverged for {language} fence {source:?}"
        );
        html
    }

    fn pair(class: &str, text: &str) -> (String, String) {
        (class.to_owned(), text.to_owned())
    }

    #[test]
    fn shell_command_position_beats_word_identity() {
        let html = preserves("pacman -S ssh pacman curl", "bash");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-command", "pacman"),
                pair("syn-flag", "-S"),
                pair("syn-argument", "ssh"),
                pair("syn-argument", "pacman"),
                pair("syn-argument", "curl"),
            ],
            "only the command position may claim a command or value class"
        );
        assert!(
            !classes(&html)
                .iter()
                .any(|class| matches!(class.as_str(), "syn-value" | "syn-env-value")),
            "positional arguments must not become values"
        );
    }

    #[test]
    fn shell_pipe_gives_each_command_a_command_position() {
        let html = preserves("curl url | jq .", "shell");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-command", "curl"),
                pair("syn-argument", "url"),
                pair("syn-command", "jq"),
                pair("syn-argument", "."),
            ]
        );
    }

    #[test]
    fn shell_flags_values_and_arguments_stay_distinct() {
        let html = preserves("grep -A 3 err log.txt", "sh");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-command", "grep"),
                pair("syn-flag", "-A"),
                pair("syn-value", "3"),
                pair("syn-argument", "err"),
                pair("syn-argument", "log.txt"),
            ],
            "a known value flag promotes exactly one argument"
        );
    }

    #[test]
    fn shell_flag_assignments_split_key_and_value() {
        let html = preserves("curl --output=out.txt -o=x url", "bash");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-command", "curl"),
                pair("syn-flag", "--output"),
                pair("syn-value", "out.txt"),
                pair("syn-flag", "-o"),
                pair("syn-value", "x"),
                pair("syn-argument", "url"),
            ]
        );
    }

    #[test]
    fn shell_quoted_flag_assignment_keeps_the_value_italic() {
        let html = preserves("curl --output=\"a b\" url", "bash");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-command", "curl"),
                pair("syn-flag", "--output"),
                pair("syn-value", "\"a b\""),
                pair("syn-argument", "url"),
            ]
        );
    }

    #[test]
    fn shell_env_prefix_marks_name_and_value() {
        let html = preserves("ENV=value cmd --needed arg", "bash");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-env", "ENV"),
                pair("syn-env-value", "value"),
                pair("syn-command", "cmd"),
                pair("syn-flag", "--needed"),
                pair("syn-argument", "arg"),
            ]
        );
    }

    #[test]
    fn shell_expansions_keep_nested_commands() {
        let html = preserves("echo \"$HOME ${HOME} $(date +%s)\"", "bash");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-command", "echo"),
                pair("syn-string", "\""),
                pair("syn-env", "$HOME ${HOME}"),
                pair("syn-command", "date"),
                pair("syn-argument", "+%s"),
                pair("syn-string", "\""),
            ],
            "expansions stay a distinct class and keep nested commands"
        );
    }

    #[test]
    fn shell_keywords_comments_and_loops() {
        let html = preserves("for f in *.md; do echo \"$f\"; done  # tail", "bash");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-keyword", "for"),
                pair("syn-env", "f"),
                pair("syn-keyword", "in"),
                pair("syn-keyword", "do"),
                pair("syn-command", "echo"),
                pair("syn-string", "\""),
                pair("syn-env", "$f"),
                pair("syn-string", "\""),
                pair("syn-keyword", "done"),
                pair("syn-comment", "# tail"),
            ]
        );
    }

    #[test]
    fn shell_heredoc_script_is_highlighted_as_shell() {
        let html = preserves(SSH_SCRIPT, "bash");
        assert_eq!(
            &spans(&html)[..4],
            &[
                pair("syn-command", "ssh"),
                pair("syn-argument", "root@host"),
                pair("syn-string", "'bash -s'"),
                pair("syn-string", "'OUTER'"),
            ],
            "the remote command line is an argument, the body is a script"
        );
        let script = spans(&html);
        for expected in [
            pair("syn-command", "set"),
            pair("syn-flag", "-e"),
            pair("syn-command", "pacman"),
            pair("syn-flag", "-S"),
            pair("syn-argument", "package"),
            pair("syn-command", "curl"),
            pair("syn-argument", "https://example.test"),
            pair("syn-command", "sh"),
            pair("syn-string", "OUTER"),
        ] {
            assert!(
                script.contains(&expected),
                "heredoc script span {expected:?} missing from {script:?}"
            );
        }
    }

    #[test]
    fn shell_quoted_heredoc_data_stays_a_string() {
        let source = "cat <<'EOF'\nhello $USER\nEOF\n";
        let html = preserves(source, "bash");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-command", "cat"),
                pair("syn-string", "'EOF'"),
                pair("syn-string", "hello $USER\nEOF"),
            ],
            "data heredoc contents must not be parsed as shell"
        );
    }

    #[test]
    fn shell_heredoc_script_nesting_is_bounded() {
        let nested = "ssh host <<'A'\nssh host <<'B'\necho deep\nB\nA\n";
        assert!(
            spans(&preserves(nested, "bash")).contains(&pair("syn-command", "echo")),
            "a nested heredoc script stays highlighted inside the bound"
        );

        let deeper = "ssh host <<'A'\nssh host <<'B'\nssh host <<'C'\nssh host <<'D'\nssh host <<'E'\necho deep\nE\nD\nC\nB\nA\n";
        let html = preserves(deeper, "bash");
        assert!(
            !spans(&html).contains(&pair("syn-command", "echo")),
            "script nesting beyond the bound stays data"
        );
    }

    #[test]
    fn shell_html_is_escaped_and_bytes_are_preserved() {
        let source = "echo '<script>alert(\"x\") & y</script>' \"héllo — 世界\"\n";
        let html = preserves(source, "bash");
        assert!(
            !html.contains("<script"),
            "source text must never form markup"
        );
        assert!(!html.contains("onmouseover="));
        assert_eq!(
            html.matches('"').count(),
            2 * html.matches("class=\"").count(),
            "every quote belongs to a generator owned attribute"
        );
    }

    #[test]
    fn shell_multiline_source_keeps_newlines() {
        let source = "if [ -f x ]; then\n  cat x\r\nfi\n";
        let html = preserves(source, "bash");
        assert_eq!(text_of(&html).matches('\n').count(), 3);
    }

    #[test]
    fn unknown_language_falls_back_to_escaped_text() {
        let html = highlighted("<b>a & b</b>", "madeuplanguage");
        assert_eq!(html, "&lt;b&gt;a &amp; b&lt;/b&gt;");
        assert!(spans(&html).is_empty());
    }

    #[test]
    fn aliases_and_case_are_normalized() {
        let canonical = highlighted("echo 'a' | tee out", "bash");
        for fence in [
            "Bash",
            "sh",
            "Shell",
            "{.bash}",
            "bash title=\"demo\"",
            "bash,ignore",
        ] {
            assert_eq!(
                highlighted("echo 'a' | tee out", fence),
                canonical,
                "fence {fence:?} must route to the shell walker"
            );
        }
    }

    #[test]
    fn rust_fence_uses_syntect_scopes() {
        let source = "fn main() {\n    let x = 1;\n}\n";
        let html = preserves(source, "rust");
        let spans = spans(&html);
        assert!(
            spans.contains(&pair("syn-storage", "fn")),
            "rust `fn` must carry a generic class: {spans:?}"
        );
        assert!(spans.contains(&pair("syn-entity", "main")));
        assert!(spans.contains(&pair("syn-number", "1")));
    }

    #[test]
    fn syntect_classes_stay_in_the_generic_namespace() {
        let html = preserves("def f(x):\n    return \"s\"  # c\n", "python");
        let allowed = [
            "syn-command",
            "syn-argument",
            "syn-flag",
            "syn-env",
            "syn-value",
            "syn-env-value",
            "syn-string",
            "syn-comment",
            "syn-keyword",
            "syn-number",
            "syn-constant",
            "syn-entity",
            "syn-support",
            "syn-storage",
            "syn-variable",
            "syn-punctuation",
            "syn-invalid",
            "syn-meta",
        ];
        for class in classes(&html) {
            assert!(
                allowed.contains(&class.as_str()),
                "unexpected class {class}"
            );
        }
        assert!(classes(&html).iter().any(|class| class == "syn-keyword"));
    }

    #[test]
    fn positional_arguments_need_known_flag_knowledge() {
        let html = preserves("weird -o x", "bash");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-command", "weird"),
                pair("syn-flag", "-o"),
                pair("syn-argument", "x"),
            ],
            "an unknown command never promotes a neighbour to a value"
        );
    }

    #[test]
    fn end_of_flags_marker_stops_flag_detection() {
        let html = preserves("ls -- -l", "bash");
        assert_eq!(
            spans(&html),
            vec![
                pair("syn-command", "ls"),
                pair("syn-argument", "--"),
                pair("syn-argument", "-l"),
            ]
        );
    }

    #[test]
    fn malformed_shell_is_reported_as_highlighted_text() {
        let sources = [
            "",
            "echo \"unterminated",
            "$(",
            "<<'X'",
            "if",
            "`",
            "echo '",
            "${",
            "$((",
            "))",
            "echo hi && ",
            "while; do",
        ];
        for source in sources {
            let html = preserves(source, "bash");
            assert!(
                html.is_empty() == source.is_empty(),
                "empty source must stay empty"
            );
        }
    }

    #[test]
    fn shell_classes_stay_within_the_contract() {
        let sources = [
            "pacman -S ssh pacman curl",
            SSH_SCRIPT,
            "ENV=value cmd --needed -abc arg",
            "for f in *.md; do echo \"$f\"; done  # tail",
            "arr=(1 2) && echo ${arr[0]:-0}",
            "[[ -f x && \"$y\" == z ]] || exit 1",
            "case $x in a) echo 1;; esac",
        ];
        let allowed = [
            "syn-command",
            "syn-argument",
            "syn-flag",
            "syn-env",
            "syn-value",
            "syn-env-value",
            "syn-string",
            "syn-comment",
            "syn-keyword",
            "syn-number",
        ];
        for source in sources {
            for class in classes(&preserves(source, "bash")) {
                assert!(
                    allowed.contains(&class.as_str()),
                    "class {class} is outside the shell contract for {source:?}"
                );
            }
        }
    }
}
