//! Synchronize Blitz layout before pointer hit-testing after Dioxus mutations.

use dioxus::prelude::*;
use dioxus_native::NodeHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

pub const WHEEL_MULTIPLIER: f64 = 4.;
pub const WHEEL_LINE_PIXELS: f64 = 20.;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollMetrics {
    pub viewport: f64,
    pub extent: f64,
    pub offset: f64,
    pub top: f64,
}

impl ScrollMetrics {
    pub fn read(handle: &NodeHandle) -> Option<Self> {
        let doc = handle.doc();
        let node = doc.get_node(handle.node_id())?;
        Some(Self {
            viewport: f64::from(node.final_layout().size.height),
            extent: f64::from(node.final_layout().scroll_height()),
            offset: node.scroll_offset().y,
            top: f64::from(node.absolute_position(0., 0.).y) + node.scroll_offset().y,
        })
    }

    pub fn thumb_length(self) -> f64 {
        (self.viewport * self.viewport / (self.viewport + self.extent).max(1.))
            .max(42.)
            .min(self.viewport)
    }

    pub fn thumb_top(self) -> f64 {
        self.offset / self.extent.max(1.) * (self.viewport - self.thumb_length())
    }
}

/// Scroll offset of one reader pane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaneScroll {
    pub x: f64,
    pub y: f64,
}

/// Scroll offsets of both reader panes, remembered per history entry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DocumentScroll {
    pub preview: PaneScroll,
    pub source: PaneScroll,
}

#[derive(Clone)]
struct ScrollbarDrag {
    handle: NodeHandle,
    grab: f64,
    onchange: EventHandler<bool>,
}

pub struct LayoutBarrier {
    mounted: RefCell<Option<NodeHandle>>,
    scrollbar_drag: RefCell<Option<ScrollbarDrag>>,
    pending: Cell<bool>,
    started: Instant,
}

impl Default for LayoutBarrier {
    fn default() -> Self {
        Self {
            mounted: RefCell::new(None),
            scrollbar_drag: RefCell::new(None),
            pending: Cell::new(true),
            started: Instant::now(),
        }
    }
}

impl LayoutBarrier {
    /// Keep scrolling in Blitz; only the hover rail is application-owned. @req DECL0001
    pub fn begin_scrollbar_drag(&self, handle: NodeHandle, y: f64, onchange: EventHandler<bool>) {
        let Some(metrics) = ScrollMetrics::read(&handle) else {
            return;
        };
        if metrics.extent <= 0.5 {
            return;
        }
        self.end_scrollbar_drag();
        let local = y - metrics.top - metrics.thumb_top();
        let grab = match (0. ..=metrics.thumb_length()).contains(&local) {
            true => local,
            false => metrics.thumb_length() / 2.,
        };
        *self.scrollbar_drag.borrow_mut() = Some(ScrollbarDrag {
            handle,
            grab,
            onchange,
        });
        self.drag_scrollbar(y);
    }

    pub fn drag_scrollbar(&self, y: f64) {
        let Some(drag) = self.scrollbar_drag.borrow().clone() else {
            return;
        };
        let Some(metrics) = ScrollMetrics::read(&drag.handle) else {
            self.end_scrollbar_drag();
            return;
        };
        let travel = (metrics.viewport - metrics.thumb_length()).max(1.);
        let offset = ((y - metrics.top - drag.grab) / travel).clamp(0., 1.) * metrics.extent;
        {
            let mut doc = drag.handle.doc_mut();
            let x = match doc.get_node(drag.handle.node_id()) {
                Some(node) => node.scroll_offset().x,
                None => return,
            };
            doc.scroll_to(
                drag.handle.node_id(),
                x,
                offset,
                blitz_dom::ScrollBehavior::Instant,
            );
            doc.shell_provider.request_redraw();
        }
        drag.onchange.call(true);
    }

    pub fn end_scrollbar_drag(&self) {
        let drag = self.scrollbar_drag.borrow_mut().take();
        if let Some(drag) = drag {
            drag.onchange.call(false);
        }
    }

    pub fn cancel_scrollbar_drag(&self, handle: &NodeHandle) {
        let owns_drag = self
            .scrollbar_drag
            .borrow()
            .as_ref()
            .is_some_and(|drag| drag.handle.node_id() == handle.node_id());
        if owns_drag {
            self.scrollbar_drag.borrow_mut().take();
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        let mounted = self.mounted.borrow();
        let doc = mounted.as_ref()?.doc();
        let input_focused = doc
            .get_focussed_node_id()
            .and_then(|id| doc.get_node(id))
            .and_then(|node| node.element_data())
            .is_some_and(|element| element.text_input_data().is_some());
        if input_focused {
            return None;
        }
        doc.get_selected_text()
    }

    pub fn scroll_tree_to(&self, y: f64) {
        if let Some(handle) = self.mounted.borrow().as_ref() {
            let mut doc = handle.doc_mut();
            if let Ok(Some(tree)) = doc.query_selector("nav.tree") {
                doc.scroll_to(tree, 0., y, blitz_dom::ScrollBehavior::Instant);
                doc.shell_provider.request_redraw();
            }
        }
    }

    /// Read both reader pane offsets; absent panes read as zero.
    pub fn read_document_scroll(&self) -> DocumentScroll {
        let mounted = self.mounted.borrow();
        let Some(handle) = mounted.as_ref() else {
            return DocumentScroll::default();
        };
        let doc = handle.doc();
        let pane = |selector: &str| {
            doc.query_selector(selector)
                .ok()
                .flatten()
                .and_then(|id| doc.get_node(id))
                .map(|node| PaneScroll {
                    x: node.scroll_offset().x,
                    y: node.scroll_offset().y,
                })
                .unwrap_or_default()
        };
        DocumentScroll {
            preview: pane(".preview"),
            source: pane(".source"),
        }
    }

    /// Restore remembered pane offsets once the panes are laid out. @req DECL0003
    /// Returns false while the target content has no layout extent yet.
    pub fn restore_document_scroll(&self, scroll: DocumentScroll) -> bool {
        self.flush();
        let mounted = self.mounted.borrow();
        let Some(handle) = mounted.as_ref() else {
            return false;
        };
        let mut doc = handle.doc_mut();
        let mut ready = true;
        for (selector, pane) in [(".preview", scroll.preview), (".source", scroll.source)] {
            if let Ok(Some(id)) = doc.query_selector(selector) {
                let extent = doc
                    .get_node(id)
                    .map(|node| f64::from(node.final_layout().scroll_height()))
                    .unwrap_or(0.);
                if pane.y > 0. && extent < pane.y {
                    ready = false;
                    continue;
                }
                doc.scroll_to(id, pane.x, pane.y, blitz_dom::ScrollBehavior::Instant);
            }
        }
        doc.shell_provider.request_redraw();
        ready
    }
    pub fn mount(&self, event: MountedEvent) {
        if let Some(handle) = event.downcast::<NodeHandle>() {
            *self.mounted.borrow_mut() = Some(handle.clone());
        }
    }

    /// Blitz beta can hit a removed paint child between DOM updates and redraw. @req DECL0001
    pub fn flush(&self) {
        if !self.pending.replace(false) {
            return;
        }
        if let Some(handle) = self.mounted.borrow().as_ref() {
            handle
                .doc_mut()
                .resolve(self.started.elapsed().as_secs_f64());
        }
    }

    /// Add the extra distance before Blitz applies its baseline wheel delta. @req DECL0001
    pub fn accelerate_wheel(&self, delta: &dioxus_native::winit::event::MouseScrollDelta) {
        use dioxus_native::winit::event::MouseScrollDelta;
        let extra = WHEEL_MULTIPLIER - 1.;
        let (x, y) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (
                f64::from(*x) * WHEEL_LINE_PIXELS * extra,
                f64::from(*y) * WHEEL_LINE_PIXELS * extra,
            ),
            MouseScrollDelta::PixelDelta(position) => (position.x * extra, position.y * extra),
            _ => return,
        };
        if let Some(handle) = self.mounted.borrow().as_ref() {
            let mut doc = handle.doc_mut();
            let target = match doc.get_hover_node_id() {
                Some(id) => id,
                None => doc.root_node().id,
            };
            if doc.scroll_node_by_has_changed(target, x, y, |_| {}) {
                doc.shell_provider.request_redraw();
            }
        }
    }
}

pub fn use_layout_barrier() -> Rc<LayoutBarrier> {
    let barrier = use_context::<Rc<LayoutBarrier>>();
    let pending = barrier.clone();
    dioxus::core::use_after_render(move || pending.pending.set(true));
    barrier
}

pub async fn clipboard(text: String) -> Result<(), String> {
    let shell = consume_context::<std::sync::Arc<dyn blitz_traits::shell::ShellProvider>>();
    match tokio::task::spawn_blocking(move || shell.set_clipboard_text(text)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err("Clipboard write failed".into()),
        Err(error) => Err(format!("Clipboard worker: {error}")),
    }
}
