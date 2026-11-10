# roxidian

A native, readonly Markdown workspace viewer built with Dioxus and Blitz.

Read rendered Markdown, inspect the exact source or keep both side by side. Browse
folders, open a folder as a workspace from its context menu, and move to the parent
workspace. SynthWave-inspired dark is the default; an Obsidian-inspired light theme
is available in the same window.
The dark palette takes its colors from Robb Owen's
[SynthWave '84](https://github.com/robb0wen/synthwave-vscode).

## Launch

```sh
nix develop -c just build-release
nix develop -c just install
rox .
rox ./doc/
rox ~/notes
rox /absolute/path/to/note.md
rox --theme light ./README.md
```

A **directory argument** becomes the workspace. A **file argument** opens that file
while preserving the terminal's current directory as workspace, including files outside
it. No argument opens the current directory. The installed launcher loads this project's
flake without changing that directory and runs the release binary through NVIDIA offload.

Documents are never written. Non-Markdown text files open as source. Raw HTML remains
literal text. UTF-8 files up to 16 MiB are accepted; binary input produces an error.
Every newly opened Markdown document starts in Preview. Source and Split are explicit
choices for the current document. Drag the sidebar or split boundary to resize panes.

In the operator's Yazi configuration, `g r` opens the hovered file or folder. Text and
source files also offer **roxidian (readonly)** in Open With. A file retains Yazi's current
directory as workspace; a folder becomes the workspace itself.

## Navigation and search

| Shortcut | Action |
|---|---|
| Ctrl+1 / Ctrl+2 / Ctrl+3 | Preview / Source / Split |
| Ctrl+F | Find in the current document; Enter / Shift+Enter select matches |
| Ctrl+Shift+F | Fuzzy search workspace file paths and text |
| Ctrl+T | Toggle theme |
| Ctrl+B | Hide/show the workspace sidebar |
| Ctrl+C | Copy selected text |
| Alt+Up | Parent workspace |
| Ctrl+R | Reload directory and document |
| Ctrl+Shift+C | Copy exact source |
| Escape | Close search or folder menu |
| Ctrl+Q | Close the application |

Workspace search includes Markdown, TOML, code and other UTF-8 text. It respects ignore
rules and excludes VCS/build caches and binary files. Each scan reads up to 10,000 files,
64 MiB total and 1 MiB per file; the UI reports skipped files and truncation. The best
100 file/content matches are shown. Select a result to open it readonly.

## Renderer

Markdown fills the reader with 5vw horizontal padding. Dark mode adds SynthWave glow;
folders use italic cyan labels. Their ancestor chain stays visible when the tree scrolls.
Markdown filenames use violet; file types have embedded SVG icons. AGENTS.md and
SKILL.md use a bold animated neon gradient. Wheel scrolling is 4× the native baseline.
Fenced blocks offer Copy and Wrap. Shell highlighting
uses syntax positions: commands are bold, positional arguments upright, flags and ENV
names use separate colours, and flag/ENV values are italic. A bounded flag catalogue
recognises separate values of common tools; unknown flags keep following words as
arguments. Other fenced languages use syntect grammars. Highlighting runs during loading.

Dioxus Native uses Blitz directly, without a WebView. Current prereleases are intentional:
`cargo update` selects compatible current releases under ordinary semver ranges.
The native renderer is experimental; the regression suite covers the observed
DOM-update/pointer hit-test failure. Use the optimized build for interactive work.

[Markdown showcase](tests/fixtures/markdown-showcase.md)

## Development

Use `nix develop -c just gate` for formatting, Clippy and tests. The flake supplies
native libraries; Cargo and rustc come from rustup.

[Behavior contract](docs/behavior.md) · [Markdown showcase](tests/fixtures/markdown-showcase.md)

## License

[Apache License 2.0](LICENSE).

Embedded icons retain their [Lucide ISC/MIT notices](THIRD_PARTY_NOTICES.md).
