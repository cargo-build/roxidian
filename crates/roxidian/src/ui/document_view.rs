//! Memoized document panes and current-document search controls.

use super::scrollbar::ScrollPane;
use super::*;

#[derive(Clone)]
pub(super) struct Opened(pub(super) Arc<Document>);
impl PartialEq for Opened {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[component]
pub(super) fn Reader(
    opened: Opened,
    mode: Mode,
    query: String,
    active: usize,
    onlink: EventHandler<String>,
    split: f64,
    onresize: EventHandler<()>,
) -> Element {
    crate::native::use_layout_barrier();
    let doc = &opened.0;
    let mode = match workspace::is_markdown(&doc.path) {
        true => mode,
        false => Mode::Source,
    };
    rsx! { div { class: if mode==Mode::Split {"panes split"} else {"panes"},
        if mode!=Mode::Preview { ScrollPane { source: true, style: if mode==Mode::Split {format!("flex: 0 0 calc({}% - 3px)",split*100.)} else {String::new()}, {render::source(&doc.source,if mode==Mode::Source {&query} else {""},active)} } }
        if mode==Mode::Split { div { class: "pane-boundary", role: "separator", aria_label: "Resize source and preview panes", onpointerdown: move |event| {event.prevent_default();onresize.call(());} } }
        if mode!=Mode::Source { ScrollPane { source: false, style: String::new(), article { class: "markdown", {render::nodes(&doc.blocks,&doc.path,onlink,&query,active)} } } }
    } }
}

#[component]
pub(super) fn FindBar(mut state: Signal<State>) -> Element {
    let snapshot = state.read();
    let text = snapshot.document.as_ref().map(|doc| {
        match (snapshot.mode, workspace::is_markdown(&doc.path)) {
            (Mode::Source, _) | (_, false) => doc.source.to_string(),
            _ => crate::document::plain(&doc.blocks),
        }
    });
    let count = text.as_ref().map_or(0, |text| {
        search::occurrences(text, &snapshot.find_query).len()
    });
    let current = match count {
        0 => 0,
        _ => snapshot.find_index % count + 1,
    };
    rsx! { div { class: "find-bar",
        input { r#type: "search", placeholder: "Find in document…", aria_label: "Find in document", value: "{snapshot.find_query}",
            onmounted: move |event| async move {if let Err(error)=event.set_focus(true).await {eprintln!("rox: focus search: {error}");}},
            oninput: move |event| {let mut state=state.write();state.find_query=event.value();state.find_index=0;},
            onkeydown: move |event| {if event.key()==Key::Named(NamedKey::Enter) && count>0 {let mut state=state.write();state.find_index=match event.modifiers().shift() {true=>(state.find_index+count-1)%count,false=>(state.find_index+1)%count};}},
        }
        span { class: "muted", "{current} / {count}" }
        button { disabled: count==0, onclick: move |_| {let mut state=state.write();state.find_index=(state.find_index+count-1)%count;}, "↑" }
        button { disabled: count==0, onclick: move |_| {let mut state=state.write();state.find_index=(state.find_index+1)%count;}, "↓" }
        button { onclick: move |_| {let mut state=state.write();state.panel=None;state.find_query.clear();}, "×" }
    } }
}
