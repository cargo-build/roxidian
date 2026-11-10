//! Fixed-height tree rows and a scroll-derived ancestor overlay for Blitz.

use super::*;

pub(super) const ROW_HEIGHT: f64 = 44.;

fn pinned_indices(rows: &[Entry], scroll: f64) -> Vec<usize> {
    let Some(first) = rows.get((scroll.max(0.) / ROW_HEIGHT) as usize) else {
        return Vec::new();
    };
    rows.iter()
        .enumerate()
        .take_while(|(index, _)| (*index as f64) * ROW_HEIGHT < scroll)
        .filter(|(_, row)| {
            row.directory && row.path != first.path && first.path.starts_with(&row.path)
        })
        .map(|(index, _)| index)
        .collect()
}

#[component]
pub(super) fn WorkspaceTree(state: Signal<State>) -> Element {
    crate::native::use_layout_barrier();
    let mut scroll = use_signal(|| 0.);
    let snapshot = state.read();
    let root = snapshot.root.display().to_string();
    let pinned = pinned_indices(&snapshot.rows, scroll());
    rsx! { div { class: "tree-container", style: "--tree-row-height: {ROW_HEIGHT}px",
        {rsx! { nav {key: "{root}", class: "tree", aria_label: "Workspace files",
            onscroll: move |event| { let y=event.scroll_top(); if scroll() != y { scroll.set(y); } },
            if snapshot.tree_loading { p {class: "muted", "Reading directory…"} }
            if snapshot.rows.is_empty() && !snapshot.tree_loading {p {class: "sidebar-hint", "No files here. Open a folder or go to its parent."} }
            for entry in &snapshot.rows { TreeRow {key:"{entry.path.display()}", state, entry:entry.clone()} }
        } }}
        if !pinned.is_empty() {div {class: "tree-sticky", aria_label:"Parent folders",
            for index in pinned { TreeRow { key:"{snapshot.rows[index].path.display()}", state, entry:snapshot.rows[index].clone(), pinned_index:Some(index), onreveal:move |y|scroll.set(y) } }
        }}
    } }
}

#[component]
fn TreeRow(
    mut state: Signal<State>,
    entry: Entry,
    pinned_index: Option<usize>,
    onreveal: Option<EventHandler<f64>>,
) -> Element {
    let kind = file_icons::Kind::for_name(&entry.name);
    let barrier = use_context::<std::rc::Rc<crate::native::LayoutBarrier>>();
    let selected = state
        .read()
        .document
        .as_ref()
        .is_some_and(|doc| doc.path == entry.path);
    let expanded = state.read().expanded.contains(&entry.path);
    let icon = match (entry.directory, expanded) {
        (true, true) => "⌄",
        (true, false) => "›",
        _ => "≡",
    };
    let class = match (entry.directory, selected) {
        (true, true) => "tree-row folder active",
        (true, false) => "tree-row folder",
        (false, true) => "tree-row active",
        _ => "tree-row",
    };
    let class = if entry.directory && entry.name.starts_with('.') {
        format!("{class} hidden-folder")
    } else {
        class.to_owned()
    };
    let class = if entry.directory {
        class
    } else {
        format!("{class} {}", kind.class())
    };
    let context = entry.clone();
    let target = entry.clone();
    rsx! {button {class, style:"padding-left: {10+entry.depth*15}px", title:"{entry.path.display()}",
        onclick:move |_| {
            if let Some(index)=pinned_index {let y=index as f64*ROW_HEIGHT;if let Some(onreveal)=onreveal {onreveal.call(y);}barrier.scroll_tree_to(y);}
            activate(state,target.clone());
        },
        oncontextmenu:move |event| {
            event.prevent_default();event.stop_propagation();
            if context.directory {let position=event.client_coordinates();state.write().menu=Some((context.path.clone(),position.x,position.y));}
        },
        if entry.directory {span {class:"icon", "{icon}"}} else {span {class:"icon file-icon",aria_hidden:true,dangerous_inner_html:kind.svg()}}
        span {class:"name",
            if !entry.directory && kind==file_icons::Kind::Guide {GuideName {name:entry.name.clone()}}
            else {"{entry.name}"}
        }
    } }
}

#[component]
fn GuideName(name: String) -> Element {
    rsx! {for (index,letter) in name.chars().enumerate() {span {class:"guide-letter",style:"animation-delay: {-0.45 * index as f64}s", "{letter}"}}}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ancestors_follow_the_visible_branch() {
        let rows = [
            ("/a", true),
            ("/a/b", true),
            ("/a/b/c.md", false),
            ("/d", true),
            ("/d/e.md", false),
        ]
        .into_iter()
        .map(|(path, directory)| Entry {
            path: PathBuf::from(path),
            name: path.into(),
            directory,
            depth: 0,
            error: None,
        })
        .collect::<Vec<_>>();
        assert!(pinned_indices(&rows, 0.).is_empty());
        assert_eq!(pinned_indices(&rows, ROW_HEIGHT * 2.), vec![0, 1]);
        assert_eq!(pinned_indices(&rows, ROW_HEIGHT * 4.), vec![3]);
        assert!(pinned_indices(&rows, 999.).is_empty());
    }
}
