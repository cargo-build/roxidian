# roxidian

Native readonly Markdown viewer: Rust 2024, Dioxus Native/Blitz, pulldown-cmark.
This repository builds independently. No parent-wrapper files are runtime or test dependencies.

- Use jj; `main` is the integration line. Read diffs with `git diff`.
- Ordinary dependency ranges and Cargo.lock; no exact versions or commit pins.
- On NixOS use `nix develop -c <command>` with the operator's rustup toolchain.
- Before landing: `nix develop -c just gate`; build release before interactive checks.
- Keep [behavior](docs/behavior.md), tests and README current with changed behavior.
- UI rendering never writes document/workspace contents or executes Markdown HTML.
- Heavy jobs use the operator's shared resource lock and task-owned systemd units.
- GUI previews preserve focus and open tiled, following the operator's desktop procedure.
