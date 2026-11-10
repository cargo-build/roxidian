# Readonly Markdown workspaces

Behavior contract: DECL0001.

`rox [PATH]` opens a native Markdown viewer. A directory PATH becomes the workspace
root. A file PATH opens that file while the invocation directory remains the workspace
root, including absolute files outside that root. With no PATH the invocation directory
is the workspace. Invalid paths produce explicit errors.

The sidebar browses directories lazily, lists text/source files, and can move the workspace
to its parent. A folder context menu offers opening that folder as the workspace.
Directory read failures remain visible and can be retried.

Documents are readonly. Preview, exact source and side-by-side source/preview use the
same UTF-8 snapshot. Loading failure is visible; reload reads the file again. Headings,
paragraphs, emphasis, lists, quotes, fenced code, links, local images, task lists and
GFM tables receive native rendering. Raw HTML is displayed as literal text; scripts
are never evaluated. Local document links resolve relative to the displayed file.

SynthWave-inspired dark is the launch default. An Obsidian-inspired white palette is
available in the same window. The UI never writes document or workspace files.

Ctrl+F searches the current rendered document. Ctrl+Shift+F performs fuzzy search across
workspace text files, including Markdown, TOML, source code and configuration files.
Results identify the file and matching text and open the selected file. Non-Markdown
text opens as readonly source. Binary files are excluded from content search.

Each newly opened Markdown file starts in Preview. Source and Split are explicit
per-document choices. Changing workspace root resets sidebar scroll. Folder labels use
italic cyan text. Sidebar and split boundaries resize by dragging. Markdown uses the
available width with 5vw horizontal padding. Wheel scrolling is twice the native baseline.
The dark theme adds SynthWave glow. Fenced code has syntax highlighting and copy/wrap
controls. Shell commands (including ssh and pacman) are bold; positional arguments,
flags, ENV names and their values use distinct styles. Commands are identified by shell
syntax, not utility names occurring in arguments. Positional arguments remain upright;
flag and ENV values are italic. Comments are muted italic text.

Ordinary Markdown line endings flow as spaces; explicit hard breaks remain line breaks.
Ctrl+C copies selected text, including with a non-Latin keyboard layout; Ctrl+Shift+C
copies the exact document source. Ctrl+B and the toolbar toggle hide/show the workspace
without discarding its expanded folders. Scrolling the tree pins the current branch's
ancestor folders above its rows; each pinned folder can collapse or open as workspace.
Hidden folder names use muted amber with a narrow dark-theme glow. All sidebar text
is twice its initial size (28px rows, 24px path, 20px heading and 22px hints); rows are
44px high with compact vertical padding, including pinned ancestors.
The document path is 18px with compact padding. Native window decorations are hidden;
small yellow/green/red controls at the toolbar's right request minimize, fullscreen
and close. The 28px workspace toggle uses directional line/chevron icons.

The implementation uses Rust 2024 and Dioxus Native with Blitz, as selected by the
operator after evaluating GPUI. Semantic HTML elements and CSS own document typography
and both palettes; no browser or WebView is required.
