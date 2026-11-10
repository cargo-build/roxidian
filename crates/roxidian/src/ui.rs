//! Dioxus window state; cancellable requests keep blocking reads off the UI executor.

use crate::{
    cli::Launch,
    document::{self, Destination, Document},
    render, search,
    theme::Theme,
    workspace::{self, Entry},
};
use dioxus::core::Task;
use dioxus::html::input_data::keyboard_types::NamedKey;
use dioxus::prelude::*;
use std::{collections::HashSet, path::PathBuf, sync::Arc};

struct State {
    root: PathBuf,
    rows: Vec<Entry>,
    expanded: HashSet<PathBuf>,
    document: Option<Arc<Document>>,
    theme: Theme,
    mode: Mode,
    file_error: Option<String>,
    tree_error: Option<String>,
    notice: Option<String>,
    loading: bool,
    tree_loading: bool,
    menu: Option<(PathBuf, f64, f64)>,
    file_task: Option<Task>,
    tree_task: Option<Task>,
    file_generation: u64,
    tree_generation: u64,
    worker: Arc<tokio::sync::Semaphore>,
    panel: Option<SearchPanel>,
    find_query: String,
    find_index: usize,
    layout: layout::Layout,
    drag: Option<layout::Boundary>,
}

#[derive(Clone, Copy, PartialEq)]
enum SearchPanel {
    Document,
    Workspace,
}

#[derive(Clone, Copy)]
enum WindowAction {
    Minimize,
    Fullscreen,
    Close,
}

impl State {
    fn new(launch: &Launch, theme: Theme, mode: Mode) -> Self {
        Self {
            root: launch.root.clone(),
            rows: Vec::new(),
            expanded: HashSet::new(),
            document: None,
            theme,
            mode,
            file_error: None,
            tree_error: None,
            notice: None,
            loading: false,
            tree_loading: false,
            menu: None,
            file_task: None,
            tree_task: None,
            file_generation: 0,
            tree_generation: 0,
            worker: Arc::new(tokio::sync::Semaphore::new(1)),
            panel: None,
            find_query: String::new(),
            find_index: 0,
            layout: layout::Layout::default(),
            drag: None,
        }
    }
}

fn load_tree(mut state: Signal<State>, root: PathBuf) {
    let (expanded, worker, generation) = {
        let mut state = state.write();
        if let Some(task) = state.tree_task.take() {
            task.cancel();
        }
        state.tree_generation += 1;
        state.tree_loading = true;
        (
            state.expanded.clone(),
            state.worker.clone(),
            state.tree_generation,
        )
    };
    let task = spawn(async move {
        let permit = match worker.acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => return,
        };
        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let result = workspace::read(&root, &expanded);
            (root, result)
        })
        .await;
        let mut state = state.write();
        if state.tree_generation != generation {
            return;
        }
        state.tree_loading = false;
        match result {
            Ok((root, Ok(rows))) => {
                state.root = root;
                state.rows = rows;
                state.tree_error = None;
            }
            Ok((_, Err(error))) => state.tree_error = Some(format!("{error:#}")),
            Err(error) => state.tree_error = Some(format!("Directory worker failed: {error}")),
        }
    });
    state.write().tree_task = Some(task);
}

fn open(mut state: Signal<State>, path: PathBuf) {
    let (worker, generation) = {
        let mut state = state.write();
        if let Some(task) = state.file_task.take() {
            task.cancel();
        }
        state.file_generation += 1;
        state.loading = true;
        state.file_error = None;
        state.notice = None;
        (state.worker.clone(), state.file_generation)
    };
    let task = spawn(async move {
        let permit = match worker.acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => return,
        };
        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            Document::load(&path)
        })
        .await;
        let mut state = state.write();
        if state.file_generation != generation {
            return;
        }
        state.loading = false;
        match result {
            Ok(Ok(document)) => {
                state.mode = Mode::for_path(&document.path);
                state.document = Some(Arc::new(document));
                state.find_index = 0;
            }
            Ok(Err(error)) => state.file_error = Some(format!("{error:#}")),
            Err(error) => state.file_error = Some(format!("Document worker failed: {error}")),
        }
    });
    state.write().file_task = Some(task);
}

fn workspace(mut state: Signal<State>, root: PathBuf) {
    {
        let mut state = state.write();
        state.expanded.clear();
        state.menu = None;
    }
    load_tree(state, root);
}

fn parent(state: Signal<State>) {
    let path = state.read().root.parent().map(std::path::Path::to_path_buf);
    if let Some(path) = path {
        workspace(state, path);
    }
}

fn reload(state: Signal<State>) {
    let (root, file) = {
        let state = state.read();
        (
            state.root.clone(),
            state.document.as_ref().map(|doc| doc.path.clone()),
        )
    };
    load_tree(state, root);
    if let Some(path) = file {
        open(state, path);
    }
}

fn set_mode(mut state: Signal<State>, mode: Mode) {
    let markdown = state
        .peek()
        .document
        .as_ref()
        .is_none_or(|doc| workspace::is_markdown(&doc.path));
    if markdown || mode == Mode::Source {
        let mut state = state.write();
        state.mode = mode;
        state.find_index = 0;
    }
}

fn copy(state: Signal<State>) {
    let source = state
        .read()
        .document
        .as_ref()
        .map(|doc| doc.source.to_string());
    if let Some(source) = source {
        copy_text(state, source, "Source copied");
    }
}

fn copy_text(mut state: Signal<State>, text: String, notice: &'static str) {
    spawn(async move {
        match crate::native::clipboard(text).await {
            Ok(()) => state.write().notice = Some(notice.into()),
            Err(error) => state.write().file_error = Some(error),
        }
    });
}

fn follow(mut state: Signal<State>, href: String) {
    let destination = state
        .read()
        .document
        .as_ref()
        .map(|doc| document::destination(&doc.path, &href));
    match destination {
        Some(Ok(Destination::Local(path))) => match path.is_dir() {
            true => workspace(state, path),
            false => open(state, path),
        },
        Some(Ok(Destination::External(url))) => {
            spawn(async move {
                match tokio::task::spawn_blocking(move || webbrowser::open(&url)).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        state.write().file_error = Some(format!("Open link: {error}"))
                    }
                    Err(error) => {
                        state.write().file_error = Some(format!("Link worker failed: {error}"))
                    }
                }
            });
        }
        Some(Ok(Destination::Anchor(_))) => {}
        Some(Err(error)) => state.write().file_error = Some(format!("{error:#}")),
        None => {}
    }
}

fn activate(mut state: Signal<State>, entry: Entry) {
    state.write().menu = None;
    match (entry.directory, entry.error.is_some()) {
        (true, _) => {
            let root = {
                let mut state = state.write();
                if !state.expanded.remove(&entry.path) {
                    state.expanded.insert(entry.path);
                }
                state.root.clone()
            };
            load_tree(state, root);
        }
        (_, true) => reload(state),
        _ => open(state, entry.path),
    }
}

fn app() -> Element {
    let barrier =
        use_context_provider(|| std::rc::Rc::new(crate::native::LayoutBarrier::default()));
    let launch = use_context::<Launch>();
    let initial_theme = use_context::<Theme>();
    let mut state = use_signal(|| State::new(&launch, initial_theme, Mode::Preview));
    use_future(move || {
        let launch = launch.clone();
        async move {
            load_tree(state, launch.root);
            if let Some(path) = launch.file {
                open(state, path);
            }
        }
    });
    let modifiers = use_hook(|| {
        std::rc::Rc::new(std::cell::Cell::new(
            dioxus_native::winit::keyboard::ModifiersState::empty(),
        ))
    });
    let window = dioxus_native::use_window();
    let chrome_window = window.clone();
    let chrome_shell = use_context::<Arc<dyn blitz_traits::shell::ShellProvider>>();
    dioxus_native::use_window_event(move |event, target| {
        use dioxus_native::winit::{
            event::{ElementState, WindowEvent},
            keyboard::{Key as NativeKey, KeyCode, NamedKey as NativeNamedKey, PhysicalKey},
        };
        match event {
            WindowEvent::PointerMoved { position, .. } => {
                barrier.flush();
                let drag = state.peek().drag;
                if let Some(boundary) = drag {
                    let scale = window.scale_factor();
                    state.write().layout.resize(
                        boundary,
                        position.x / scale,
                        f64::from(window.surface_size().width) / scale,
                    );
                }
            }
            WindowEvent::PointerButton {
                state: ElementState::Released,
                ..
            } => {
                barrier.flush();
                if state.peek().drag.is_some() {
                    state.write().drag = None;
                }
            }
            WindowEvent::PointerButton { .. } | WindowEvent::PointerEntered { .. } => {
                barrier.flush()
            }
            WindowEvent::MouseWheel { delta, .. } => {
                barrier.flush();
                barrier.accelerate_wheel(delta);
            }
            WindowEvent::ModifiersChanged(changed) => modifiers.set(changed.state()),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if modifiers.get().control_key()
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyC)
                {
                    if modifiers.get().shift_key() {
                        copy(state);
                    } else if let Some(text) = barrier.selected_text() {
                        copy_text(state, text, "Selection copied");
                    }
                    return;
                }
                match &event.logical_key {
                    NativeKey::Character(key) if modifiers.get().control_key() => {
                        match key.to_lowercase().as_str() {
                            "q" => target.exit(),
                            "1" => set_mode(state, Mode::Preview),
                            "2" => set_mode(state, Mode::Source),
                            "3" => set_mode(state, Mode::Split),
                            "t" => {
                                let theme = state.peek().theme.toggle();
                                state.write().theme = theme;
                            }
                            "r" => reload(state),
                            "b" => {
                                let visible = state.peek().layout.sidebar_visible;
                                state.write().layout.sidebar_visible = !visible;
                            }
                            "f" => {
                                state.write().panel = Some(if modifiers.get().shift_key() {
                                    SearchPanel::Workspace
                                } else {
                                    SearchPanel::Document
                                })
                            }
                            _ => {}
                        }
                    }
                    NativeKey::Named(NativeNamedKey::ArrowUp) if modifiers.get().alt_key() => {
                        parent(state)
                    }
                    NativeKey::Named(NativeNamedKey::Escape) => {
                        let mut state = state.write();
                        state.menu = None;
                        state.panel = None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    });
    rsx! { Shell {state, onwindow:move |action|match action {
        WindowAction::Close=>chrome_shell.request_window_close(),
        WindowAction::Minimize=>chrome_window.set_minimized(true),
        WindowAction::Fullscreen=>{let screen=match chrome_window.fullscreen(){Some(_)=>None,None=>Some(dioxus_native::winit::monitor::Fullscreen::Borderless(None))};chrome_window.set_fullscreen(screen);}
    }} }
}

#[component]
fn Shell(mut state: Signal<State>, onwindow: Option<EventHandler<WindowAction>>) -> Element {
    let barrier = crate::native::use_layout_barrier();
    let snapshot = state.read();
    let document = snapshot.document.clone();
    let root = snapshot.root.display().to_string();
    let mode = snapshot.mode;
    let theme = snapshot.theme;
    let onlink = use_callback(move |href| follow(state, href));
    let source_only = document
        .as_ref()
        .is_some_and(|doc| !workspace::is_markdown(&doc.path));
    let copy_barrier = barrier.clone();
    rsx! {
        style { {include_str!("viewer.css")} }
        div { class: theme.class(), tabindex: "-1",
            onkeydown: move |event| {
                use dioxus::html::input_data::keyboard_types::Code;
                if event.modifiers().ctrl() && event.code() == Code::KeyC &&
                    (event.modifiers().shift() || copy_barrier.selected_text().is_some()) { event.prevent_default(); }
            },
            onmounted: move |event| barrier.mount(event),
            onclick: move |_| {if state.peek().menu.is_some() {state.write().menu=None;}},
            header { class: "toolbar",
                button { class:"workspace-toggle", title: "Toggle workspace (Ctrl+B)", aria_label: "Toggle workspace", aria_expanded: snapshot.layout.sidebar_visible,
                    onclick: move |_| {let visible=state.peek().layout.sidebar_visible;state.write().layout.sidebar_visible = !visible;},
                    svg {width:"28",height:"28",view_box:"0 0 24 24",fill:"none",stroke:"currentColor",stroke_width:"1.8",stroke_linecap:"round",stroke_linejoin:"round",
                        path {d:if snapshot.layout.sidebar_visible {"M3 6h10M3 12h7M3 18h10M20 8l-4 4 4 4"} else {"M11 6h10M14 12h7M11 18h10M4 8l4 4-4 4"}}
                    }
                }
                span { class: "brand", "◈ roxidian" }
                button { class: if mode==Mode::Preview {"active"} else {""}, disabled: source_only, onclick: move |_| set_mode(state,Mode::Preview), "Preview" }
                button { class: if mode==Mode::Source {"active"} else {""}, onclick: move |_| set_mode(state,Mode::Source), "Source" }
                button { class: if mode==Mode::Split {"active"} else {""}, disabled: source_only, onclick: move |_| set_mode(state,Mode::Split), "Split" }
                span { class: "spacer" }
                button { onclick: move |_| state.write().panel=Some(SearchPanel::Document), "Find" }
                button { onclick: move |_| state.write().panel=Some(SearchPanel::Workspace), "Search workspace" }
                button { onclick: move |_| copy(state), disabled: document.is_none(), "Copy source" }
                button { onclick: move |_| reload(state), "↻ Reload" }
                button { onclick: move |_| state.write().theme=theme.toggle(), "{theme.name()}" }
                div {class:"window-controls",
                    button {class:"window-control minimize",title:"Minimize",aria_label:"Minimize",onclick:move |_|{if let Some(action)=onwindow {action.call(WindowAction::Minimize);}},span {class:"window-dot"}}
                    button {class:"window-control fullscreen",title:"Toggle full screen",aria_label:"Toggle full screen",onclick:move |_|{if let Some(action)=onwindow {action.call(WindowAction::Fullscreen);}},span {class:"window-dot"}}
                    button {class:"window-control close",title:"Close",aria_label:"Close",onclick:move |_|{if let Some(action)=onwindow {action.call(WindowAction::Close);}},span {class:"window-dot"}}
                }
            }
            if let Some(error)=&snapshot.file_error { div { class: "banner error", "{error}" } }
            if let Some(error)=&snapshot.tree_error { div { class: "banner error", "{error}" } }
            if snapshot.loading { div { class: "banner", "Loading document…" } }
            if let Some(notice)=&snapshot.notice { div { class: "banner", "{notice}" } }
            div { class: "layout",
                if snapshot.layout.sidebar_visible {
                aside { class: "sidebar", style: "width: {snapshot.layout.sidebar}px",
                    div { class: "sidebar-heading", span { class: "eyebrow", "WORKSPACE" } button { onclick: move |_| parent(state), disabled: snapshot.root.parent().is_none(), "↑ Parent" } }
                    div { class: "root-path", "{root}" }
                    {rsx! {tree::WorkspaceTree {key:"{root}",state}}}
                    div { class: "sidebar-hint", "Right-click a folder to open it as workspace" }
                }
                div { class: "pane-boundary", role: "separator", aria_label: "Resize workspace sidebar",
                    onpointerdown: move |event| {event.prevent_default();state.write().drag=Some(layout::Boundary::Sidebar);},
                }
                }
                main { class: "reader",
                    if snapshot.panel==Some(SearchPanel::Document) { FindBar {state} }
                    if snapshot.panel==Some(SearchPanel::Workspace) { WorkspaceSearch {state} }
                    if let Some(doc)=&document {
                        div { class: "document-path", "{doc.path.display()}" }
                        Reader { opened: Opened(doc.clone()), mode, query: snapshot.find_query.clone(), active: snapshot.find_index, onlink, split: snapshot.layout.split, onresize: move |_| state.write().drag=Some(layout::Boundary::Split) }
                        footer { class: "statusbar", span { class: "readonly", "READ ONLY" } span { "{doc.lines} lines · {doc.source.len()} bytes · UTF-8" } }
                    } else {
                        div { class: "empty", h1 { "Read. Explore. Stay in flow." } p { "Choose a Markdown file from the workspace." } p { "Ctrl+1 Preview · Ctrl+2 Source · Ctrl+3 Split · Ctrl+T Theme" } }
                    }
                }
            }
            if let Some((path,x,y))=&snapshot.menu {
                {let path=path.clone();let label=path.display().to_string();
                rsx! { div { class: "context-menu", role: "menu", style: "left: min({x}px, calc(100vw - 260px)); top: min({y}px, calc(100vh - 100px))", onclick: move |event| event.stop_propagation(),
                    div { class: "context-path", "{label}" }
                    button { role: "menuitem", onclick: move |_| workspace(state,path.clone()), "Open as workspace" }
                } }}
            }
        }
    }
}

pub fn run(launch: Launch, theme: Theme) {
    let attributes = dioxus_native::WindowAttributes::default()
        .with_title("roxidian")
        .with_decorations(false)
        .with_surface_size(dioxus_native::LogicalSize::new(1440., 960.))
        .with_min_surface_size(dioxus_native::LogicalSize::new(820., 480.));
    #[cfg(target_os = "linux")]
    let attributes = attributes.with_platform_attributes(Box::new(
        dioxus_native::winit::platform::wayland::WindowAttributesWayland::default()
            .with_name("roxidian", "roxidian"),
    ));
    dioxus_native::launch_cfg(
        app,
        vec![
            Box::new(move || Box::new(launch.clone())),
            Box::new(move || Box::new(theme)),
        ],
        vec![Box::new(attributes)],
    );
}

mod document_view;
mod layout;
mod tree;
use layout::Mode;
#[cfg(test)]
mod tests;
mod workspace_search;
use document_view::{FindBar, Opened, Reader};
use workspace_search::WorkspaceSearch;
