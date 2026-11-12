//! Native DOM mutation and pointer regression.

use super::*;
use blitz_dom::Document as _;
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
struct TestClipboard(std::sync::Mutex<Option<String>>);

impl blitz_traits::shell::ShellProvider for TestClipboard {
    fn set_clipboard_text(&self, text: String) -> Result<(), blitz_traits::shell::ClipboardError> {
        *self
            .0
            .lock()
            .map_err(|_| blitz_traits::shell::ClipboardError)? = Some(text);
        Ok(())
    }
    fn get_clipboard_text(&self) -> Result<String, blitz_traits::shell::ClipboardError> {
        self.0
            .lock()
            .map_err(|_| blitz_traits::shell::ClipboardError)?
            .clone()
            .ok_or(blitz_traits::shell::ClipboardError)
    }
}

struct Harness {
    doc: dioxus_native::DioxusDocument,
    state: Signal<State>,
    barrier: Rc<crate::native::LayoutBarrier>,
}

#[test]
fn standalone_source_shows_syntax_and_search_matches() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("main.rs");
    std::fs::write(&path, "fn main() { let value = 42; }\n").expect("source fixture");
    let Harness {
        mut doc,
        mut state,
        barrier,
    } = harness();
    doc.vdom.in_scope(ScopeId::APP, || {
        let mut state = state.write();
        state.mode = Mode::Source;
        state.document = Some(Arc::new(Document::load(&path).expect("source document")));
    });
    doc.poll(None);
    barrier.flush();
    assert!(
        !doc.inner
            .borrow()
            .query_selector_all("pre.source .syn-keyword")
            .expect("selector")
            .is_empty()
    );
    doc.vdom
        .in_scope(ScopeId::APP, || state.write().find_query = "value".into());
    doc.poll(None);
    barrier.flush();
    assert_eq!(
        doc.inner
            .borrow()
            .query_selector_all("pre.source mark")
            .expect("selector")
            .len(),
        1
    );
    assert!(
        !doc.inner
            .borrow()
            .query_selector_all("pre.source .syn-keyword")
            .expect("selector")
            .is_empty()
    );
    doc.vdom
        .in_scope(ScopeId::APP, || state.write().find_query = "fn main".into());
    doc.poll(None);
    barrier.flush();
    assert!(
        doc.inner
            .borrow()
            .query_selector_all("pre.source mark")
            .expect("selector")
            .len()
            >= 2
    );
    assert_eq!(
        state.peek().document.as_ref().expect("document").source,
        "fn main() { let value = 42; }\n"
    );
}

fn harness() -> Harness {
    fn fixture() -> Element {
        let slot = use_context::<Rc<Cell<Option<Signal<State>>>>>();
        let state = use_signal(|| {
            let mut state = State::new(
                &Launch {
                    root: PathBuf::from("/tmp"),
                    file: None,
                },
                Theme::Synthwave,
                Mode::Split,
            );
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/markdown-showcase.md");
            state.document = Some(Arc::new(Document::load(&path).expect("showcase fixture")));
            state
        });
        slot.set(Some(state));
        rsx! {Shell {state}}
    }
    let slot: Rc<Cell<Option<Signal<State>>>> = Rc::new(Cell::new(None));
    let barrier = Rc::new(crate::native::LayoutBarrier::default());
    let vdom = VirtualDom::new(fixture);
    let clipboard: Arc<dyn blitz_traits::shell::ShellProvider> = Arc::new(TestClipboard::default());
    vdom.provide_root_context(clipboard.clone());
    vdom.provide_root_context(slot.clone());
    vdom.provide_root_context(barrier.clone());
    let mut doc = dioxus_native::DioxusDocument::new(
        vdom,
        dioxus_native::DocumentConfig {
            shell_provider: Some(clipboard),
            html_parser_provider: Some(Arc::new(blitz_html::HtmlProvider)),
            viewport: Some(blitz_traits::shell::Viewport::new(
                960,
                1050,
                1.,
                blitz_traits::shell::ColorScheme::Dark,
            )),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.inner.borrow_mut().resolve(0.);
    let state = slot.get().expect("mounted state");
    Harness {
        doc,
        state,
        barrier,
    }
}

#[test]
fn tree_pins_ancestors_and_sidebar_can_be_hidden() {
    use blitz_traits::events::{BlitzWheelDelta, BlitzWheelEvent, UiEvent};
    let Harness {
        mut doc,
        mut state,
        barrier,
    } = harness();
    let entry = |path: &str, directory, depth| Entry {
        path: PathBuf::from(path),
        name: path.into(),
        directory,
        depth,
        error: None,
    };
    doc.vdom.in_scope(ScopeId::APP, || {
        let mut state = state.write();
        state.rows = std::iter::once(entry("/a", true, 0))
            .chain(std::iter::once(entry("/a/b", true, 1)))
            .chain((0..100).map(|i| entry(&format!("/a/b/file-{i}"), false, 2)))
            .collect();
        state.expanded = [PathBuf::from("/a"), PathBuf::from("/a/b")]
            .into_iter()
            .collect();
    });
    doc.poll(None);
    barrier.flush();
    let tree = doc
        .inner
        .borrow()
        .query_selector("nav.tree")
        .expect("selector")
        .expect("tree");
    let pointer = doc
        .inner
        .borrow()
        .get_node(tree)
        .expect("tree")
        .synthetic_click_event_data(Default::default());
    doc.handle_ui_event(UiEvent::PointerMove(pointer.clone()));
    doc.handle_ui_event(UiEvent::Wheel(BlitzWheelEvent {
        delta: BlitzWheelDelta::Pixels(0., -240.),
        coords: pointer.coords,
        buttons: Default::default(),
        mods: Default::default(),
        element: Default::default(),
    }));
    doc.poll(None);
    barrier.flush();
    assert_eq!(
        doc.inner
            .borrow()
            .query_selector_all(".tree-sticky .folder")
            .expect("selector")
            .len(),
        2
    );
    doc.vdom.in_scope(ScopeId::APP, || {
        state.write().layout.sidebar_visible = false
    });
    doc.poll(None);
    barrier.flush();
    assert!(
        doc.inner
            .borrow()
            .query_selector("nav.tree")
            .expect("selector")
            .is_none()
    );
    doc.vdom
        .in_scope(ScopeId::APP, || state.write().layout.sidebar_visible = true);
    doc.poll(None);
    barrier.flush();
    assert_eq!(state.read().expanded.len(), 2);
    assert_eq!(
        doc.inner
            .borrow()
            .query_selector_all("nav.tree .tree-row")
            .expect("selector")
            .len(),
        102
    );
}

#[test]
fn guide_names_animate_in_the_native_style_engine_and_files_have_svg_icons() {
    let Harness {
        mut doc,
        mut state,
        barrier,
    } = harness();
    doc.vdom.in_scope(ScopeId::APP, || {
        state.write().rows = ["AGENTS.md", "SKILL.md", "README.md"]
            .into_iter()
            .map(|name| Entry {
                path: PathBuf::from("/tmp").join(name),
                name: name.into(),
                directory: false,
                depth: 0,
                error: None,
            })
            .collect();
    });
    doc.poll(None);
    barrier.flush();
    let letters = doc
        .inner
        .borrow()
        .query_selector_all(".guide-letter")
        .expect("selector");
    assert_eq!(letters.len(), 17);
    assert_eq!(
        doc.inner
            .borrow()
            .query_selector_all(".file-icon svg")
            .expect("selector")
            .len(),
        3
    );
    assert_eq!(
        doc.inner
            .borrow()
            .query_selector_all(".file-markdown .name")
            .expect("selector")
            .len(),
        1
    );
    let colour = || {
        format!(
            "{:?}",
            doc.inner
                .borrow()
                .get_node(letters[0])
                .expect("letter")
                .primary_styles()
                .expect("style")
                .clone_color()
        )
    };
    doc.inner.borrow_mut().resolve(0.);
    let before = colour();
    doc.inner.borrow_mut().resolve(2.);
    let after = colour();
    assert_ne!(before, after, "guide text must actually animate in Blitz");
}

#[test]
fn selection_copy_uses_selected_text_instead_of_the_whole_source() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            let Harness {
                mut doc,
                state,
                barrier,
            } = harness();
            let heading = doc
                .inner
                .borrow()
                .query_selector("h1")
                .expect("selector")
                .expect("heading");
            doc.inner
                .borrow_mut()
                .set_text_selection(heading, 0, heading, 8);
            let selected = barrier.selected_text().expect("selected heading");
            assert_eq!(selected, "roxidian");
            doc.vdom.in_scope(ScopeId::APP, || {
                copy_text(state, selected, "Selection copied")
            });
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                loop {
                    doc.poll(None);
                    if doc
                        .inner
                        .borrow()
                        .shell_provider
                        .get_clipboard_text()
                        .ok()
                        .as_deref()
                        == Some("roxidian")
                    {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                }
            })
            .await
            .expect("clipboard receipt");
        });
}

#[test]
fn reader_hit_test_during_mode_changes() {
    let Harness {
        mut doc,
        mut state,
        barrier,
    } = harness();
    [Mode::Source, Mode::Split, Mode::Preview]
        .into_iter()
        .for_each(|mode| {
            let started = std::time::Instant::now();
            doc.vdom
                .in_scope(ScopeId::APP, || state.write().mode = mode);
            doc.poll(None);
            barrier.flush();
            eprintln!("native {mode:?} update/layout: {:?}", started.elapsed());
            (0..1050).step_by(20).for_each(|y| {
                (0..960).step_by(20).for_each(|x| {
                    let _ = doc.inner.borrow().hit(x as f32, y as f32);
                })
            });
            doc.inner.borrow_mut().resolve(0.);
        });
    assert_eq!(
        doc.inner
            .borrow()
            .query_selector_all("thead th")
            .expect("selector")
            .len(),
        2
    );
    assert!(
        doc.inner
            .borrow()
            .query_selector_all("script")
            .expect("selector")
            .is_empty()
    );
    assert!(
        doc.inner
            .borrow()
            .query_selector_all(".code-block .syn-command")
            .expect("selector")
            .len()
            > 5
    );
    assert!(
        doc.inner
            .borrow_mut()
            .scroll_to_fragment("explore-without-editing")
    );
    doc.vdom
        .in_scope(ScopeId::APP, || state.write().find_query = "pacman".into());
    doc.poll(None);
    barrier.flush();
    assert!(
        !doc.inner
            .borrow()
            .query_selector_all(".code-block mark")
            .expect("selector")
            .is_empty()
    );
    doc.vdom
        .in_scope(ScopeId::APP, || state.write().find_query.clear());
    doc.poll(None);
    barrier.flush();
    assert!(
        !doc.inner
            .borrow()
            .query_selector_all(".code-block .syn-command")
            .expect("selector")
            .is_empty()
    );
    let _ = doc.inner.borrow().hit(700., 500.);
}

#[test]
fn changing_workspace_resets_tree_scroll() {
    let Harness {
        mut doc,
        mut state,
        barrier,
    } = harness();
    let rows = |count: usize| {
        (0..count)
            .map(|index| Entry {
                path: PathBuf::from(format!("/tmp/entry-{index}")),
                name: format!("entry-{index}"),
                directory: false,
                depth: 0,
                error: None,
            })
            .collect::<Vec<_>>()
    };
    doc.vdom.in_scope(ScopeId::APP, || {
        let mut state = state.write();
        state.root = PathBuf::from("/tmp/child");
        state.rows = rows(150);
    });
    doc.poll(None);
    barrier.flush();
    let tree = doc
        .inner
        .borrow()
        .query_selector("nav.tree")
        .expect("selector")
        .expect("tree");
    doc.inner
        .borrow_mut()
        .scroll_to(tree, 0., 3000., blitz_dom::ScrollBehavior::Instant);
    assert!(
        doc.inner
            .borrow()
            .get_node(tree)
            .expect("tree node")
            .scroll_offset()
            .y
            > 100.
    );
    doc.vdom.in_scope(ScopeId::APP, || {
        let mut state = state.write();
        state.root = PathBuf::from("/tmp");
        state.rows = rows(3);
    });
    doc.poll(None);
    barrier.flush();
    let tree = doc
        .inner
        .borrow()
        .query_selector("nav.tree")
        .expect("selector")
        .expect("tree");
    assert_eq!(
        doc.inner
            .borrow()
            .get_node(tree)
            .expect("tree node")
            .scroll_offset()
            .y,
        0.
    );
}

#[test]
fn pane_geometry_changes_without_changing_document_padding() {
    let Harness {
        mut doc,
        mut state,
        barrier,
    } = harness();
    doc.vdom.in_scope(ScopeId::APP, || {
        let mut state = state.write();
        state.layout.resize(layout::Boundary::Sidebar, 350., 960.);
        state.layout.split = 0.7;
    });
    doc.poll(None);
    barrier.flush();
    let inner = doc.inner.borrow();
    let sidebar = inner
        .query_selector("aside.sidebar")
        .expect("selector")
        .expect("sidebar");
    let node = inner.get_node(sidebar).expect("sidebar node");
    assert!((node.final_layout().size.width - 350.).abs() < 1.);
    let article = inner
        .query_selector("article.markdown")
        .expect("selector")
        .expect("article");
    let node = inner.get_node(article).expect("article node");
    assert!((node.final_layout().padding.left - 48.).abs() < 1.);
}

#[test]
fn accelerated_wheel_uses_the_requested_multiplier() {
    use blitz_traits::events::{BlitzWheelDelta, BlitzWheelEvent, UiEvent};
    let Harness {
        mut doc,
        mut state,
        barrier,
    } = harness();
    doc.vdom.in_scope(ScopeId::APP, || {
        let mut state = state.write();
        state.mode = Mode::Source;
        let source = "long line\n".repeat(250);
        state.document = Some(Arc::new(Document {
            path: PathBuf::from("/tmp/test.md"),
            source,
            source_syntax: None,
            source_html: None,
            lines: 250,
            blocks: Vec::new(),
        }));
    });
    doc.poll(None);
    barrier.flush();
    let source = doc
        .inner
        .borrow()
        .query_selector("pre.source")
        .expect("selector")
        .expect("source");
    let pointer = doc
        .inner
        .borrow()
        .get_node(source)
        .expect("source node")
        .synthetic_click_event_data(Default::default());
    doc.handle_ui_event(UiEvent::PointerMove(pointer.clone()));
    barrier.accelerate_wheel(&dioxus_native::winit::event::MouseScrollDelta::PixelDelta(
        dioxus_native::winit::dpi::PhysicalPosition::new(0., -50.),
    ));
    doc.handle_ui_event(UiEvent::Wheel(BlitzWheelEvent {
        delta: BlitzWheelDelta::Pixels(0., -50.),
        coords: pointer.coords,
        buttons: Default::default(),
        mods: Default::default(),
        element: Default::default(),
    }));
    assert_eq!(
        doc.inner
            .borrow()
            .get_node(source)
            .expect("source node")
            .scroll_offset()
            .y,
        200.
    );
}

#[test]
fn document_scrollbar_reveals_before_wheel_and_drags_each_pane() {
    use blitz_traits::events::{BlitzWheelDelta, BlitzWheelEvent, MouseEventButton, UiEvent};
    [Mode::Preview, Mode::Source, Mode::Split]
        .into_iter()
        .for_each(|mode| {
            let Harness {
                mut doc,
                mut state,
                barrier,
            } = harness();
            doc.vdom
                .in_scope(ScopeId::APP, || state.write().mode = mode);
            doc.poll(None);
            barrier.flush();
            let panes = doc
                .inner
                .borrow()
                .query_selector_all(".document-pane")
                .expect("panes");
            assert_eq!(panes.len(), if mode == Mode::Split { 2 } else { 1 });
            panes.into_iter().for_each(|pane| {
                let (content, rail) = {
                    let inner = doc.inner.borrow();
                    let children = &inner.get_node(pane).expect("pane").children;
                    (children[0], children[1])
                };
                assert_eq!(
                    doc.inner
                        .borrow()
                        .get_node(content)
                        .expect("content")
                        .scroll_offset()
                        .y,
                    0.
                );
                let pointer = doc
                    .inner
                    .borrow()
                    .get_node(rail)
                    .expect("rail")
                    .synthetic_click_event_data(Default::default());
                assert_eq!(
                    doc.inner
                        .borrow()
                        .get_node(rail)
                        .expect("rail")
                        .primary_styles()
                        .expect("style")
                        .clone_opacity(),
                    0.
                );
                doc.handle_ui_event(UiEvent::PointerMove(pointer));
                doc.poll(None);
                barrier.flush();
                assert_eq!(
                    doc.inner
                        .borrow()
                        .get_node(rail)
                        .expect("rail")
                        .primary_styles()
                        .expect("style")
                        .clone_opacity(),
                    1.
                );
                let thumb = doc.inner.borrow().get_node(rail).expect("rail").children[0];
                let mut pointer = doc
                    .inner
                    .borrow()
                    .get_node(thumb)
                    .expect("thumb")
                    .synthetic_click_event_data(Default::default());
                pointer.button = MouseEventButton::Main;
                doc.handle_ui_event(UiEvent::PointerDown(pointer.clone()));
                barrier.drag_scrollbar(f64::from(pointer.coords.client_y) + 150.);
                doc.poll(None);
                barrier.flush();
                let offset = doc
                    .inner
                    .borrow()
                    .get_node(content)
                    .expect("content")
                    .scroll_offset()
                    .y;
                assert!(offset > 0., "{mode:?} must scroll when its thumb moves");
                barrier.end_scrollbar_drag();
                barrier.drag_scrollbar(f64::from(pointer.coords.client_y) + 300.);
                assert_eq!(
                    doc.inner
                        .borrow()
                        .get_node(content)
                        .expect("content")
                        .scroll_offset()
                        .y,
                    offset,
                    "release ends dragging"
                );
                doc.handle_ui_event(UiEvent::PointerUp(pointer.clone()));
                doc.handle_ui_event(UiEvent::Wheel(BlitzWheelEvent {
                    delta: BlitzWheelDelta::Pixels(0., -50.),
                    coords: pointer.coords,
                    buttons: Default::default(),
                    mods: Default::default(),
                    element: Default::default(),
                }));
                let inner = doc.inner.borrow();
                let node = inner.get_node(content).expect("content");
                assert_eq!(
                    node.scroll_offset().y,
                    (offset + 200.).min(f64::from(node.final_layout().scroll_height())),
                    "wheel still works over the rail"
                );
            });
        });
}

#[test]
fn external_delete_removes_the_tree_row() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("gone.md");
    std::fs::write(&path, "# Gone\n").expect("fixture");
    let Harness {
        mut doc,
        mut state,
        barrier,
    } = harness();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");
    let _guard = runtime.enter();
    doc.vdom.in_scope(ScopeId::APP, || {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        state.write().watch = Some(crate::watch::Watch::new(tx).expect("watcher"));
        watch_loop(state, rx);
        load_tree(state, directory.path().to_path_buf());
    });
    let row_present = |doc: &dioxus_native::DioxusDocument| {
        doc.inner
            .borrow()
            .query_selector("nav.tree .tree-row")
            .expect("selector")
            .is_some()
    };
    pump_until(&mut doc, &runtime, &barrier, || {
        state.peek().rows.iter().any(|row| row.path == path)
    });
    assert!(
        row_present(&doc),
        "fixture row must render before the delete"
    );
    std::fs::remove_file(&path).expect("external delete");
    pump_until(&mut doc, &runtime, &barrier, || {
        !state.peek().rows.iter().any(|row| row.path == path)
    });
    assert!(
        !row_present(&doc),
        "deleted row must leave the tree without manual reload"
    );
}

fn pump_until(
    doc: &mut dioxus_native::DioxusDocument,
    runtime: &tokio::runtime::Runtime,
    barrier: &Rc<crate::native::LayoutBarrier>,
    condition: impl Fn() -> bool,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "external change never reached the tree"
        );
        doc.poll(None);
        barrier.flush();
        runtime.block_on(tokio::time::sleep(std::time::Duration::from_millis(20)));
    }
    doc.poll(None);
    barrier.flush();
}

#[test]
fn history_back_and_forward_restore_documents_and_truncate() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let a = directory.path().join("a.md");
    let b = directory.path().join("b.md");
    let c = directory.path().join("c.md");
    for path in [&a, &b, &c] {
        std::fs::write(path, "# Doc\n").expect("fixture");
    }
    let Harness {
        mut doc,
        state,
        barrier,
    } = harness();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");
    let _guard = runtime.enter();
    let at = |path: &PathBuf| {
        let path = path.clone();
        move || {
            state
                .peek()
                .document
                .as_ref()
                .is_some_and(|doc| doc.path == path)
        }
    };
    doc.vdom.in_scope(ScopeId::APP, || visit(state, a.clone()));
    pump_until(&mut doc, &runtime, &barrier, at(&a));
    doc.vdom.in_scope(ScopeId::APP, || visit(state, b.clone()));
    pump_until(&mut doc, &runtime, &barrier, at(&b));
    assert!(can_back(&state.peek()));
    assert!(!can_forward(&state.peek()));
    doc.vdom.in_scope(ScopeId::APP, || back(state));
    pump_until(&mut doc, &runtime, &barrier, at(&a));
    assert!(can_forward(&state.peek()));
    doc.vdom.in_scope(ScopeId::APP, || forward(state));
    pump_until(&mut doc, &runtime, &barrier, at(&b));
    doc.vdom.in_scope(ScopeId::APP, || back(state));
    pump_until(&mut doc, &runtime, &barrier, at(&a));
    doc.vdom.in_scope(ScopeId::APP, || visit(state, c.clone()));
    pump_until(&mut doc, &runtime, &barrier, at(&c));
    assert!(
        !can_forward(&state.peek()),
        "a new visit truncates the forward branch"
    );
    assert!(
        doc.inner
            .borrow()
            .query_selector("button[aria-label=\"Back\"]")
            .expect("selector")
            .is_some()
    );
}
