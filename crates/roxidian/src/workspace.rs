//! Lazy directory snapshots; only expanded branches are read.

use anyhow::{Context, Result};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
    pub depth: usize,
    pub error: Option<String>,
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["md", "markdown", "mdown", "mkd", "mdx"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

pub fn read(root: &Path, expanded: &HashSet<PathBuf>) -> Result<Vec<Entry>> {
    let mut rows = Vec::new();
    append(root, expanded, 0, &mut rows)?;
    Ok(rows)
}

fn append(
    root: &Path,
    expanded: &HashSet<PathBuf>,
    depth: usize,
    rows: &mut Vec<Entry>,
) -> Result<()> {
    let mut entries = fs::read_dir(root)
        .with_context(|| format!("read {}", root.display()))?
        .enumerate()
        .filter_map(|(index, entry)| {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    return Some(Entry {
                        path: root.to_path_buf(),
                        name: format!("Unreadable entry {index}: {error}"),
                        directory: false,
                        depth,
                        error: Some(error.to_string()),
                    });
                }
            };
            let path = entry.path();
            match fs::metadata(&path) {
                Ok(metadata) if metadata.is_dir() || metadata.is_file() => Some(Entry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path,
                    directory: metadata.is_dir(),
                    depth,
                    error: None,
                }),
                Ok(_) => None,
                Err(error) => Some(Entry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path,
                    directory: false,
                    depth,
                    error: Some(error.to_string()),
                }),
            }
        })
        .collect::<Vec<_>>();
    entries.retain(|entry| !matches!(entry.name.as_str(), ".git" | ".jj"));
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then(a.name.cmp(&b.name))
    });
    entries.into_iter().for_each(|entry| {
        let descend = entry.directory && expanded.contains(&entry.path);
        let path = entry.path.clone();
        rows.push(entry);
        if descend && let Err(error) = append(&path, expanded, depth + 1, rows) {
            rows.push(Entry {
                path,
                name: format!("{error:#}"),
                directory: false,
                depth: depth + 1,
                error: Some(format!("{error:#}")),
            });
        }
    });
    Ok(())
}
