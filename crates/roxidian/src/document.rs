//! Immutable UTF-8 source and CommonMark/GFM tree share one load lifetime.

use anyhow::{Context, Result, bail};
use pulldown_cmark::{Event, Options, Parser, Tag};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

/// Bounds accidental binary/huge-file loads without changing document contents. @req DECL0001
pub const MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub enum Node {
    Element(Tag<'static>, Vec<Node>),
    Text(String),
    Code(String),
    Syntax { source: String, html: String },
    Html(String),
    Break,
    Rule,
    Task(bool),
}

#[derive(Debug)]
pub struct Document {
    pub path: PathBuf,
    pub source: String,
    pub lines: usize,
    pub blocks: Vec<Node>,
}

impl Document {
    pub fn load(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("resolve {}", path.display()))?;
        if !std::fs::metadata(&path)?.is_file() {
            bail!("{} is not a regular file", path.display());
        }
        let file = File::open(&path).with_context(|| format!("read {}", path.display()))?;
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            bail!("file exceeds the 16 MiB viewing limit");
        }
        let source = String::from_utf8(bytes).context("document must be UTF-8")?;
        if source.contains('\0') {
            bail!("binary content cannot be viewed as Markdown");
        }
        let blocks = match crate::workspace::is_markdown(&path) {
            true => parse(&source),
            false => Vec::new(),
        };
        let lines = source.lines().count();
        Ok(Self {
            path,
            source,
            lines,
            blocks,
        })
    }
}

pub fn parse(source: &str) -> Vec<Node> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut roots = Vec::new();
    let mut stack: Vec<(Tag<'static>, Vec<Node>)> = Vec::new();
    Parser::new_ext(source, options).for_each(|event| {
        let node = match event {
            Event::Start(tag) => {
                stack.push((tag.into_static(), Vec::new()));
                return;
            }
            Event::End(_) => match stack.pop() {
                Some((Tag::Item, children)) => Node::Element(Tag::Item, group_item(children)),
                Some((tag @ Tag::CodeBlock(_), children)) => {
                    let source = plain(&children);
                    let language = match &tag {
                        Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(language)) => {
                            language.as_ref()
                        }
                        _ => "text",
                    };
                    let children = match crate::syntax::highlight(&source, language) {
                        Ok(html) => vec![Node::Syntax { source, html }],
                        Err(error) => {
                            eprintln!("rox: highlight {language}: {error:#}");
                            children
                        }
                    };
                    Node::Element(tag, children)
                }
                Some((tag, children)) => Node::Element(tag, children),
                None => return,
            },
            Event::Text(text) => Node::Text(text.into_string()),
            Event::Code(text) => Node::Code(text.into_string()),
            Event::Html(text) | Event::InlineHtml(text) => Node::Html(text.into_string()),
            Event::SoftBreak => Node::Text(" ".into()),
            Event::HardBreak => Node::Break,
            Event::Rule => Node::Rule,
            Event::TaskListMarker(checked) => Node::Task(checked),
            _ => return,
        };
        match stack.last_mut() {
            Some((_, children)) => children.push(node),
            None => roots.push(node),
        }
    });
    roots
}

pub fn plain(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            Node::Element(_, children) => plain(children),
            Node::Text(text) | Node::Html(text) | Node::Code(text) => text.clone(),
            Node::Syntax { source, .. } => source.clone(),
            Node::Break => "\n".into(),
            Node::Task(true) => "☑ ".into(),
            Node::Task(false) => "☐ ".into(),
            Node::Rule => String::new(),
        })
        .collect()
}

fn group_item(children: Vec<Node>) -> Vec<Node> {
    let mut result = Vec::new();
    let mut inline = Vec::new();
    children.into_iter().for_each(|node| {
        let block = matches!(
            &node,
            Node::Element(
                Tag::Paragraph | Tag::List(_) | Tag::CodeBlock(_) | Tag::BlockQuote(_),
                _
            )
        );
        if block {
            if !inline.is_empty() {
                result.push(Node::Element(Tag::Paragraph, std::mem::take(&mut inline)));
            }
            result.push(node);
        } else {
            inline.push(node);
        }
    });
    if !inline.is_empty() {
        result.push(Node::Element(Tag::Paragraph, inline));
    }
    result
}

#[derive(Debug, PartialEq, Eq)]
pub enum Destination {
    External(String),
    Local(PathBuf),
    Anchor(String),
}

pub fn destination(document: &Path, href: &str) -> Result<Destination> {
    if let Some(anchor) = href.strip_prefix('#') {
        return Ok(Destination::Anchor(anchor.to_owned()));
    }
    let base = url::Url::from_file_path(document)
        .map_err(|_| anyhow::anyhow!("document path is not absolute"))?;
    let url = base.join(href).context("invalid link")?;
    match url.scheme() {
        "http" | "https" | "mailto" => Ok(Destination::External(url.into())),
        "file" => Ok(Destination::Local(
            url.to_file_path()
                .map_err(|_| anyhow::anyhow!("invalid file link"))?,
        )),
        scheme => bail!("unsupported link scheme: {scheme}"),
    }
}
