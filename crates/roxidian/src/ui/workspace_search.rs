//! Debounced workspace search owns its query and results locally.

use super::*;

#[component]
pub(super) fn WorkspaceSearch(mut state: Signal<State>) -> Element {
    crate::native::use_layout_barrier();
    let mut query = use_signal(String::new);
    let mut results = use_signal(search::Results::default);
    let mut loading = use_signal(|| false);
    let root = state.read().root.clone();
    let worker = state.read().worker.clone();
    use_resource(move || {
        let query = query();
        let root = root.clone();
        let worker = worker.clone();
        async move {
            loading.set(true);
            tokio::time::sleep(std::time::Duration::from_millis(180)).await;
            let permit = match worker.acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => return,
            };
            match tokio::task::spawn_blocking(move || {
                let _permit = permit;
                search::workspace(&root, &query)
            })
            .await
            {
                Ok(value) => results.set(value),
                Err(error) => {
                    state.write().file_error = Some(format!("Search worker failed: {error}"))
                }
            }
            loading.set(false);
        }
    });
    let results = results.read();
    rsx! { section { class: "workspace-search", aria_label: "Workspace fuzzy search",
        div { class: "find-bar",
            input { r#type: "search", placeholder: "Fuzzy search files and text…", aria_label: "Workspace query", value: "{query}",
                onmounted: move |event| async move {if let Err(error)=event.set_focus(true).await {eprintln!("rox: focus search: {error}");}},
                oninput: move |event| query.set(event.value()),
            }
            button { onclick: move |_| state.write().panel=None, "×" }
        }
        div { class: "search-summary", if loading() {"Searching… "} "{results.hits.len()} results · {results.scanned} files · {results.skipped} skipped" if results.truncated {" · scan limit reached"} }
        div { class: "search-results",
            for hit in &results.hits {
                {let path=hit.path.clone();let line=hit.line;let excerpt=hit.excerpt.clone();
                rsx! { button { class: "search-result", key: "{hit.path.display()}",
                    onclick: move |_| {
                        {let mut state=state.write();state.panel=None;state.find_index=0;state.find_query=if line.is_some() {excerpt.trim().to_owned()} else {String::new()};}
                        visit(state,path.clone());
                    },
                    strong { "{hit.path.display()}" if let Some(line)=line {":{line}"} }
                    span { "{hit.excerpt}" }
                } }}
            }
        }
    } }
}
