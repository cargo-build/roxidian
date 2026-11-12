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

External filesystem changes refresh the view without manual reload: the tree follows
created, removed and renamed entries in the workspace root and expanded folders, and
the open document re-reads when its file or an ancestor folder changes. Watching never
writes files. A document deleted outside keeps its last snapshot with a visible error,
as manual reload does; the chosen view mode survives an automatic refresh.

Following a document link, tree row, search result or launch file records a visit.
Toolbar back/forward arrows and Alt+Left/Alt+Right move through that history like a
browser, restoring the document and the view mode chosen when it was left; a new visit
clears the forward branch. Reload and external refresh add no history entries.

SynthWave-inspired dark is the launch default. An Obsidian-inspired white palette is
available in the same window. The UI never writes document or workspace files.

Ctrl+F searches the current rendered document. Ctrl+Shift+F performs fuzzy search across
workspace text files, including Markdown, TOML, source code and configuration files.
Results identify the file and matching text and open the selected file. Non-Markdown
text opens as readonly source. Binary files are excluded from content search.
Standalone source and configuration files use syntax highlighting where a grammar
is available. Search and clipboard actions continue to use the exact source text.

Each newly opened Markdown file starts in Preview. Source and Split are explicit
per-document choices. Changing workspace root resets sidebar scroll. Folder labels use
italic cyan text. Sidebar and split boundaries resize by dragging. Markdown uses the
available width with 5vw horizontal padding. Wheel scrolling is four times the native baseline.
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
Files use embedded type-specific SVG icons. Markdown filenames have a distinct violet
colour. AGENTS.md and SKILL.md are bold with a glowing animated multicolour gradient;
reduced-motion mode keeps a static gradient.
Preview body text is 26.4px and fenced code is 23.1px (1.65 times their initial sizes).
Heading sizes remain unchanged; the exact-source pane keeps its independent typography.
Each document pane offers a right-edge scrollbar on pointer hover, even before
wheel scrolling. Dragging its thumb scrolls that pane and release ends the drag.

Published desktop distributions use glibc with a bundled Nix runtime definition. Static
musl cannot load the Vulkan/windowing libraries required by this native renderer.
Versioned local installation lives in ~/.local/roxidian/<version>; the rox command
selects the current installed version while preserving the caller's working directory.
Installation prepares the Nix library closure for that version. Installed launches
use it directly and keep the caller's arguments and GPU offload behavior.

The implementation uses Rust 2024 and Dioxus Native with Blitz, as selected by the
operator after evaluating GPUI. Semantic HTML elements and CSS own document typography
and both palettes; no browser or WebView is required.
