//! Hover rails expose scrolling before Blitz's wheel-triggered scrollbar appears.

use crate::native::{ScrollMetrics, WHEEL_LINE_PIXELS, WHEEL_MULTIPLIER, use_layout_barrier};
use dioxus::prelude::*;
use dioxus_native::NodeHandle;

#[component]
pub(super) fn ScrollPane(source: bool, style: String, children: Element) -> Element {
    let barrier = use_layout_barrier();
    let mut handle = use_signal(|| None::<NodeHandle>);
    let mut metrics = use_signal(ScrollMetrics::default);
    let mut dragging = use_signal(|| false);
    let sync = EventHandler::new(move |active: bool| {
        if *dragging.peek() != active {
            dragging.set(active);
        }
        let current = handle.read().as_ref().and_then(ScrollMetrics::read);
        if let Some(current) = current
            && *metrics.peek() != current
        {
            metrics.set(current);
        }
    });
    let mounted = move |event: MountedEvent| {
        if let Some(node) = event.downcast::<NodeHandle>() {
            handle.set(Some(node.clone()));
        }
    };
    let cleanup = barrier.clone();
    use_drop(move || {
        if let Some(node) = handle.peek().as_ref() {
            cleanup.cancel_scrollbar_drag(node);
        }
    });
    let geometry = metrics();
    let rail = match dragging() {
        true => "scrollbar-rail dragging",
        false => "scrollbar-rail",
    };
    rsx! {
        div { class: "document-pane", style,
            if source {
                pre { class: "source", tabindex: "0", aria_label: "Readonly source",
                    onmounted: mounted, onscroll: move |_| sync.call(*dragging.peek()), {children} }
            } else {
                section { class: "preview", tabindex: "0", aria_label: "Markdown preview",
                    onmounted: mounted, onscroll: move |_| sync.call(*dragging.peek()), {children} }
            }
            div { class: rail, aria_hidden: true,
                onpointerenter: move |_| sync.call(*dragging.peek()),
                onpointermove: move |_| sync.call(*dragging.peek()),
                onwheel: move |event| {
                    event.prevent_default();
                    let delta = event.delta();
                    let scale = match delta {
                        dioxus::html::geometry::WheelDelta::Lines(_) => WHEEL_LINE_PIXELS,
                        dioxus::html::geometry::WheelDelta::Pages(_) => metrics.peek().viewport,
                        _ => 1.,
                    } * WHEEL_MULTIPLIER;
                    let delta = delta.strip_units();
                    let node = handle.read().clone();
                    if let Some(node) = node {
                        {
                            let mut doc = node.doc_mut();
                            doc.scroll_node_by_has_changed(node.node_id(), delta.x * scale, delta.y * scale, |_| {});
                            doc.shell_provider.request_redraw();
                        }
                        sync.call(*dragging.peek());
                    }
                },
                onpointerdown: move |event| {
                    if event.trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary) { return; }
                    let node = handle.read().clone();
                    if let Some(node) = node {
                        event.prevent_default();
                        event.stop_propagation();
                        barrier.begin_scrollbar_drag(node, event.client_coordinates().y, sync);
                    }
                },
                if geometry.extent > 0.5 {
                    div { class: "scrollbar-thumb", style: "top: {geometry.thumb_top()}px; height: {geometry.thumb_length()}px" }
                }
            }
        }
    }
}
