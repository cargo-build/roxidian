//! Non-recursive watches over the directories whose rows are visible.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{collections::HashSet, path::PathBuf};

/// Watcher results reach the UI loop; errors stay visible as a banner.
struct Forward(tokio::sync::mpsc::UnboundedSender<notify::Result<Event>>);

impl notify::EventHandler for Forward {
    fn handle_event(&mut self, event: notify::Result<Event>) {
        let _ = self.0.send(event);
    }
}

pub struct Watch {
    watcher: RecommendedWatcher,
    watched: HashSet<PathBuf>,
}

impl Watch {
    pub fn new(
        events: tokio::sync::mpsc::UnboundedSender<notify::Result<Event>>,
    ) -> notify::Result<Self> {
        Ok(Self {
            watcher: RecommendedWatcher::new(Forward(events), notify::Config::default())?,
            watched: HashSet::new(),
        })
    }

    /// Match watches to the visible directories; vanished ones drop out.
    pub fn sync(&mut self, dirs: impl IntoIterator<Item = PathBuf>) -> notify::Result<()> {
        let desired: HashSet<PathBuf> = dirs.into_iter().filter(|path| path.is_dir()).collect();
        for path in self.watched.difference(&desired) {
            // A deleted directory already lost its kernel watch.
            let _ = self.watcher.unwatch(path);
        }
        for path in desired.difference(&self.watched) {
            self.watcher.watch(path, RecursiveMode::NonRecursive)?;
        }
        self.watched = desired;
        Ok(())
    }
}

/// Which refreshes a batch of external events requires.
pub struct Refresh {
    pub tree: bool,
    pub paths: Vec<PathBuf>,
}

pub fn summarize(events: &[Event]) -> Refresh {
    let mut refresh = Refresh {
        tree: false,
        paths: Vec::new(),
    };
    for event in events {
        refresh.tree |= matches!(
            event.kind,
            EventKind::Create(_)
                | EventKind::Remove(_)
                | EventKind::Modify(notify::event::ModifyKind::Name(_))
        );
        refresh.paths.extend(event.paths.iter().cloned());
    }
    refresh
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn visible_directory_changes_arrive_and_unwatch_silences() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let mut watch = Watch::new(tx).expect("watcher");
            watch.sync([directory.path().to_path_buf()]).expect("sync");
            let file = directory.path().join("note.md");
            let renamed = directory.path().join("renamed.md");
            std::fs::write(&file, "hello").expect("write");
            let event = next_kind(&mut rx, |event| {
                matches!(event.kind, EventKind::Create(_)) && event.paths.contains(&file)
            })
            .await;
            assert!(event.paths.contains(&file));
            std::fs::rename(&file, &renamed).expect("rename");
            let event = next_kind(&mut rx, |event| {
                matches!(
                    event.kind,
                    EventKind::Modify(notify::event::ModifyKind::Name(_))
                ) && event.paths.contains(&renamed)
            })
            .await;
            assert!(event.paths.contains(&renamed));
            std::fs::remove_file(&renamed).expect("remove");
            let event = next_kind(&mut rx, |event| {
                matches!(event.kind, EventKind::Remove(_)) && event.paths.contains(&renamed)
            })
            .await;
            assert!(event.paths.contains(&renamed));
            while matches!(
                tokio::time::timeout(Duration::from_millis(200), rx.recv()).await,
                Ok(Some(_))
            ) {}
            watch.sync([]).expect("unwatch");
            std::fs::write(&file, "again").expect("write");
            assert!(
                tokio::time::timeout(Duration::from_millis(300), rx.recv())
                    .await
                    .is_err()
            );
        });
    }

    /// One filesystem operation emits several inotify events; scan for the match.
    async fn next_kind(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<notify::Result<Event>>,
        wanted: impl Fn(&Event) -> bool,
    ) -> Event {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            let event = tokio::time::timeout_at(deadline, rx.recv())
                .await
                .expect("event")
                .expect("channel")
                .expect("event");
            if wanted(&event) {
                return event;
            }
        }
    }

    #[test]
    fn summarize_separates_tree_changes_from_content_paths() {
        let content = Event::new(EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Content,
        )))
        .add_path(PathBuf::from("/w/a.md"));
        let remove = Event::new(EventKind::Remove(notify::event::RemoveKind::Folder))
            .add_path(PathBuf::from("/w/sub"));
        let refresh = summarize(&[content.clone(), remove]);
        assert!(refresh.tree);
        assert_eq!(
            refresh.paths,
            vec![PathBuf::from("/w/a.md"), PathBuf::from("/w/sub")]
        );
        assert!(!summarize(&[content]).tree);
    }
}
