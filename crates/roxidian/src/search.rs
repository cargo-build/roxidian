//! Bounded fuzzy text search; binary files and ignored build/VCS trees stay out.

use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_SCAN_BYTES: usize = 64 * 1024 * 1024;
const MAX_FILES: usize = 10_000;
const MAX_RESULTS: usize = 100;

#[derive(Clone, Debug)]
pub struct Hit {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub excerpt: String,
    pub score: u32,
}

#[derive(Default, Debug)]
pub struct Results {
    pub hits: Vec<Hit>,
    pub scanned: usize,
    pub skipped: usize,
    pub truncated: bool,
}

/// UTF-8 byte ranges retain the original spelling and never split a character. @req DECL0001
pub fn occurrences(text: &str, query: &str) -> Vec<std::ops::Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let pattern = regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build();
    match pattern {
        Ok(pattern) => pattern.find_iter(text).map(|m| m.range()).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn workspace(root: &Path, query: &str) -> Results {
    let mut result = Results::default();
    if query.trim().is_empty() {
        return result;
    }
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buffer = Vec::new();
    let mut total_bytes = 0;
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| {
            !matches!(
                entry.file_name().to_str(),
                Some(".git" | ".jj" | "target" | "node_modules" | ".direnv" | ".cache")
            )
        })
        .build();
    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        if result.scanned >= MAX_FILES || total_bytes >= MAX_SCAN_BYTES {
            result.truncated = true;
            break;
        }
        let path = entry.path();
        let file = match File::open(path) {
            Ok(file) => file,
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };
        let mut bytes = Vec::new();
        if file
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .is_err()
        {
            result.skipped += 1;
            continue;
        }
        total_bytes += bytes.len();
        result.scanned += 1;
        if bytes.len() as u64 > MAX_FILE_BYTES || bytes.contains(&0) {
            result.skipped += 1;
            continue;
        }
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };
        let relative = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
        let filename_score = pattern.score(Utf32Str::new(&relative, &mut buffer), &mut matcher);
        let content = text
            .lines()
            .enumerate()
            .filter_map(|(line, text)| {
                pattern
                    .score(Utf32Str::new(text, &mut buffer), &mut matcher)
                    .map(|score| (score, line + 1, text))
            })
            .max_by_key(|(score, _, _)| *score)
            .map(|(score, line, text)| {
                let mut indices = Vec::new();
                pattern.indices(Utf32Str::new(text, &mut buffer), &mut matcher, &mut indices);
                let start = match indices.into_iter().min() {
                    Some(index) => (index as usize).saturating_sub(40),
                    None => 0,
                };
                let excerpt = text.chars().skip(start).take(260).collect::<String>();
                (score, line, excerpt)
            });
        let file_wins = filename_score.is_some_and(|score| {
            content
                .as_ref()
                .is_none_or(|(content_score, _, _)| score >= *content_score)
        });
        match (filename_score, content) {
            (Some(score), _) if file_wins => {
                result.hits.push(Hit {
                    path: path.to_path_buf(),
                    line: None,
                    excerpt: relative.into_owned(),
                    score,
                });
            }
            (_, Some((score, line, excerpt))) => result.hits.push(Hit {
                path: path.to_path_buf(),
                line: Some(line),
                excerpt,
                score,
            }),
            _ => {}
        }
        if result.hits.len() > MAX_RESULTS * 2 {
            rank(&mut result.hits);
            result.hits.truncate(MAX_RESULTS);
        }
    }
    rank(&mut result.hits);
    result.hits.truncate(MAX_RESULTS);
    result
}

fn rank(hits: &mut [Hit]) {
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.path.cmp(&b.path))
            .then(a.line.cmp(&b.line))
    });
}
