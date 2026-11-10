//! Semantic HTML through RSX: source text never becomes executable markup.

use crate::document::{self, Destination, Node, plain};
use dioxus::prelude::*;
use pulldown_cmark::{HeadingLevel, Tag};

struct Highlights {
    ranges: Vec<std::ops::Range<usize>>,
    offset: usize,
    active: usize,
}

#[component]
fn CodeBlock(language: String, source: String, children: Element) -> Element {
    crate::native::use_layout_barrier();
    let mut wrap = use_signal(|| false);
    let mut message = use_signal(|| "Copy".to_owned());
    rsx! { div { class: if wrap() {"code-block wrapped"} else {"code-block"},
        div { class: "code-language", span {"{language}"} span {class:"spacer"}
            button {class:if wrap(){"active"}else{""}, onclick:move |_|wrap.toggle(), "Wrap"}
            button {onclick:move |_|{let source=source.clone();spawn(async move {message.set(match crate::native::clipboard(source).await {Ok(())=>"Copied".into(),Err(error)=>error});});}, "{message}"}
        }
        pre { code { {children} } }
    } }
}

impl Highlights {
    fn text(&mut self, text: &str) -> Element {
        let start = self.offset;
        self.offset += text.len();
        let mut cursor = 0;
        let mut pieces = Vec::new();
        self.ranges.iter().enumerate().filter(|(_,range)|range.start<self.offset && range.end>start).for_each(|(index,range)| {
            let left=range.start.saturating_sub(start);let right=(range.end-start).min(text.len());
            let prefix=&text[cursor..left];let value=&text[left..right];let active=index==self.active;
            pieces.push(rsx! { Fragment { key: "{index}-{start}-{active}", "{prefix}" mark { class: if active {"current-match"} else {""},
                onmounted: move |mounted| async move {if active && let Err(error)=mounted.scroll_to(dioxus::html::ScrollBehavior::Instant).await {eprintln!("rox: scroll to match: {error}");}},
                "{value}" } }
            });
            cursor=right;
        });
        let suffix = &text[cursor..];
        rsx! { {pieces.into_iter()} "{suffix}" }
    }
}

pub fn nodes(
    nodes: &[Node],
    path: &Path,
    follow: EventHandler<String>,
    query: &str,
    active: usize,
) -> Element {
    let mut highlights = Highlights {
        ranges: crate::search::occurrences(&plain(nodes), query),
        offset: 0,
        active,
    };
    walk(nodes, path, follow, &mut highlights, false)
}

pub fn source(text: &str, query: &str, active: usize) -> Element {
    Highlights {
        ranges: crate::search::occurrences(text, query),
        offset: 0,
        active,
    }
    .text(text)
}
use std::path::Path;

pub fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|ch| match ch {
            ' ' | '-' => Some('-'),
            ch if ch.is_alphanumeric() || ch == '_' => Some(ch),
            _ => None,
        })
        .collect()
}

fn walk(
    nodes: &[Node],
    path: &Path,
    follow: EventHandler<String>,
    highlights: &mut Highlights,
    header: bool,
) -> Element {
    rsx! { for (index,node) in nodes.iter().enumerate() { Fragment { key: "{index}", {element(node,path,follow,highlights,header)} } } }
}

fn element(
    node: &Node,
    path: &Path,
    follow: EventHandler<String>,
    highlights: &mut Highlights,
    header: bool,
) -> Element {
    match node {
        Node::Text(text) | Node::Html(text) => highlights.text(text),
        Node::Code(text) => rsx! { code { {highlights.text(text)} } },
        Node::Syntax { source, html } => {
            let end = highlights.offset + source.len();
            if highlights
                .ranges
                .iter()
                .any(|range| range.start < end && range.end > highlights.offset)
            {
                return highlights.text(source);
            }
            highlights.offset = end;
            rsx! { span { dangerous_inner_html: html.clone() } }
        }
        Node::Break => {
            highlights.offset += 1;
            rsx! { br {} }
        }
        Node::Rule => rsx! { hr {} },
        Node::Task(checked) => {
            highlights.offset += "☑ ".len();
            rsx! { input { r#type: "checkbox", checked: *checked, disabled: true, aria_label: "Task status" } " " }
        }
        Node::Element(tag, children) => {
            let content = walk(
                children,
                path,
                follow,
                highlights,
                header || matches!(tag, Tag::TableHead),
            );
            match tag {
                Tag::Paragraph => rsx! { p { {content} } },
                Tag::Heading { level, .. } => {
                    let id = slug(&plain(children));
                    match level {
                        HeadingLevel::H1 => rsx! { h1 { id, {content} } },
                        HeadingLevel::H2 => rsx! { h2 { id, {content} } },
                        HeadingLevel::H3 => rsx! { h3 { id, {content} } },
                        HeadingLevel::H4 => rsx! { h4 { id, {content} } },
                        HeadingLevel::H5 => rsx! { h5 { id, {content} } },
                        HeadingLevel::H6 => rsx! { h6 { id, {content} } },
                    }
                }
                Tag::Strong => rsx! { strong { {content} } },
                Tag::Emphasis => rsx! { em { {content} } },
                Tag::Strikethrough => rsx! { del { {content} } },
                Tag::BlockQuote(_) => rsx! { blockquote { {content} } },
                Tag::CodeBlock(kind) => {
                    let language = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(language) => language.as_ref(),
                        _ => "text",
                    };
                    let source = plain(children);
                    rsx! { CodeBlock {language:language.to_owned(),source,children:content} }
                }
                Tag::List(Some(start)) => rsx! { ol { start: "{start}", {content} } },
                Tag::List(None) => rsx! { ul { {content} } },
                Tag::Item => rsx! { li { {content} } },
                Tag::Table(_) => rsx! { div { class: "table-scroll", table { {content} } } },
                Tag::TableHead => rsx! { thead { tr { {content} } } },
                Tag::TableRow => rsx! { tr { {content} } },
                Tag::TableCell if header => rsx! { th { {content} } },
                Tag::TableCell => rsx! { td { {content} } },
                Tag::Link {
                    dest_url, title, ..
                } => {
                    let href = dest_url.to_string();
                    rsx! { a { href: "{dest_url}", title: "{title}", onclick: move |event| {
                        if !href.starts_with('#') { event.prevent_default(); follow.call(href.clone()); }
                    }, {content} } }
                }
                Tag::Image {
                    dest_url, title, ..
                } => {
                    let src = match document::destination(path, dest_url) {
                        Ok(Destination::Local(path)) => {
                            url::Url::from_file_path(path).ok().map(String::from)
                        }
                        Ok(Destination::External(url))
                            if url.starts_with("https://") || url.starts_with("http://") =>
                        {
                            Some(url)
                        }
                        _ => None,
                    };
                    let alt = plain(children);
                    match src {
                        Some(src) => rsx! { img { src, alt, title: "{title}" } },
                        None => rsx! { span { class: "muted", "[Image: {alt}]" } },
                    }
                }
                _ => content,
            }
        }
    }
}
