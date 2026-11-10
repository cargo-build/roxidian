//! File kinds select embedded Lucide SVGs; guide filenames keep priority over Markdown.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Guide,
    Markdown,
    Rust,
    Nix,
    Config,
    Json,
    Yaml,
    Script,
    Code,
    Database,
    Image,
    Archive,
    Lock,
    Git,
    Package,
    Text,
    Generic,
}

impl Kind {
    pub fn for_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "agents.md" | "skill.md" => return Self::Guide,
            ".gitignore" | ".gitattributes" | ".gitmodules" => return Self::Git,
            "cargo.toml" | "package.json" => return Self::Package,
            ".env" | ".editorconfig" => return Self::Config,
            "license" | "copying" => return Self::Text,
            "makefile" | "justfile" | ".envrc" => return Self::Script,
            _ => {}
        }
        if crate::workspace::is_markdown(std::path::Path::new(name)) {
            return Self::Markdown;
        }
        match lower.rsplit_once('.').map(|(_, extension)| extension) {
            Some("rs") => Self::Rust,
            Some("nix") => Self::Nix,
            Some("toml" | "ini" | "conf" | "cfg" | "kdl") => Self::Config,
            Some("json" | "jsonc" | "jsonl" | "ndjson") => Self::Json,
            Some("yaml" | "yml") => Self::Yaml,
            Some("sh" | "bash" | "fish" | "zsh" | "nu") => Self::Script,
            Some(
                "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "c" | "h" | "cpp" | "hpp" | "css"
                | "html" | "lua" | "rb",
            ) => Self::Code,
            Some("sql" | "db" | "sqlite" | "sqlite3") => Self::Database,
            Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico") => Self::Image,
            Some("zip" | "gz" | "xz" | "zst" | "tar" | "7z" | "rar") => Self::Archive,
            Some("lock") => Self::Lock,
            Some("txt" | "log" | "csv" | "tsv") => Self::Text,
            Some("deb" | "rpm" | "apk" | "crate") => Self::Package,
            _ => Self::Generic,
        }
    }

    pub fn class(self) -> &'static str {
        match self {
            Self::Guide => "file-guide",
            Self::Markdown => "file-markdown",
            Self::Rust => "file-rust",
            Self::Nix => "file-nix",
            Self::Config => "file-config",
            Self::Json => "file-json",
            Self::Yaml => "file-yaml",
            Self::Script => "file-script",
            Self::Code => "file-code",
            Self::Database => "file-database",
            Self::Image => "file-image",
            Self::Archive => "file-archive",
            Self::Lock => "file-lock",
            Self::Git => "file-git",
            Self::Package => "file-package",
            Self::Text => "file-text",
            Self::Generic => "file-generic",
        }
    }

    pub fn svg(self) -> &'static str {
        match self {
            Self::Guide => include_str!("../../../../assets/file-icons/sparkles.svg"),
            Self::Markdown => include_str!("../../../../assets/file-icons/book-open.svg"),
            Self::Rust => include_str!("../../../../assets/file-icons/cog.svg"),
            Self::Nix => include_str!("../../../../assets/file-icons/snowflake.svg"),
            Self::Config => include_str!("../../../../assets/file-icons/settings.svg"),
            Self::Json => include_str!("../../../../assets/file-icons/braces.svg"),
            Self::Yaml => include_str!("../../../../assets/file-icons/hash.svg"),
            Self::Script => include_str!("../../../../assets/file-icons/terminal.svg"),
            Self::Code => include_str!("../../../../assets/file-icons/file-code.svg"),
            Self::Database => include_str!("../../../../assets/file-icons/database.svg"),
            Self::Image => include_str!("../../../../assets/file-icons/image.svg"),
            Self::Archive => include_str!("../../../../assets/file-icons/archive.svg"),
            Self::Lock => include_str!("../../../../assets/file-icons/lock-keyhole.svg"),
            Self::Git => include_str!("../../../../assets/file-icons/git-branch.svg"),
            Self::Package => include_str!("../../../../assets/file-icons/package.svg"),
            Self::Text => include_str!("../../../../assets/file-icons/file-text.svg"),
            Self::Generic => include_str!("../../../../assets/file-icons/file.svg"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guide_names_take_priority_and_markdown_aliases_share_one_rule() {
        assert_eq!(Kind::for_name("AGENTS.md"), Kind::Guide);
        assert_eq!(Kind::for_name("skill.md"), Kind::Guide);
        assert_eq!(Kind::for_name("README.MDOWN"), Kind::Markdown);
        assert_eq!(Kind::for_name("SKILL.md.bak"), Kind::Generic);
        assert_eq!(Kind::for_name("Cargo.lock"), Kind::Lock);
        assert_eq!(Kind::for_name("flake.nix"), Kind::Nix);
        assert_ne!(
            Kind::for_name("main.rs").svg(),
            Kind::for_name("config.toml").svg()
        );
    }
}
