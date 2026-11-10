//! Synchronize Blitz layout before pointer hit-testing after Dioxus mutations.

use dioxus::prelude::*;
use dioxus_native::NodeHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

pub const WHEEL_MULTIPLIER: f64 = 4.;

pub struct LayoutBarrier {
    mounted: RefCell<Option<NodeHandle>>,
    pending: Cell<bool>,
    started: Instant,
}

impl Default for LayoutBarrier {
    fn default() -> Self {
        Self {
            mounted: RefCell::new(None),
            pending: Cell::new(true),
            started: Instant::now(),
        }
    }
}

impl LayoutBarrier {
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
            MouseScrollDelta::LineDelta(x, y) => {
                (f64::from(*x) * 20. * extra, f64::from(*y) * 20. * extra)
            }
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
