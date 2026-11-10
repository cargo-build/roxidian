//! Invocation paths retain the terminal's workspace for file arguments.

use anyhow::{Context, Result, bail};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "rox", version, about = "Readonly Markdown workspace viewer")]
pub struct Arguments {
    /// Directory becomes workspace; a file retains the invocation directory as workspace.
    pub path: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t=crate::theme::Theme::default())]
    pub theme: crate::theme::Theme,
}
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub root: PathBuf,
    pub file: Option<PathBuf>,
}

impl Launch {
    pub fn resolve(cwd: &Path, argument: Option<&Path>) -> Result<Self> {
        let cwd = cwd.canonicalize().context("resolve invocation directory")?;
        let path = match argument {
            Some(path) => cwd.join(path),
            None => cwd.clone(),
        };
        let path = path
            .canonicalize()
            .with_context(|| format!("open {}", path.display()))?;
        match (path.is_dir(), path.is_file()) {
            (true, _) => Ok(Self {
                root: path,
                file: None,
            }),
            (_, true) => Ok(Self {
                root: cwd,
                file: Some(path),
            }),
            _ => bail!(
                "{} is neither a directory nor a regular file",
                path.display()
            ),
        }
    }
}
