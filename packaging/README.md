# roxidian desktop distribution

This is the optimized x86_64 NixOS/glibc desktop build. Nix supplies its runtime
libraries; the executable and licenses stay in this directory. Vulkan/Wayland/X11
libraries load dynamically, so this GUI is not distributed as a static musl binary.

Install from the extracted archive:

```sh
./install.sh
~/.local/bin/rox --version
rox .
rox /absolute/path/to/note.md
```

The installer verifies SHA256SUMS, copies the versioned directory into
~/.local/roxidian/ and switches its current symlink. It refuses a different build
already installed under the same version. The existing rox entry may be a symlink;
an unrelated regular executable is never overwritten. Repeating installation of
the same verified build is safe.

For a separate prefix, set ROXIDIAN_INSTALL_ROOT and ROXIDIAN_BIN_DIR before invoking
install.sh. Relative prefixes resolve from invocation CWD. Each prefix can select
its current version independently; simultaneous installs into one root serialize.

Installation resolves the bundled Nix runtime before activating the version.
The .runtime symlink keeps those libraries reachable during Nix garbage collection.
The launcher uses the prepared libraries directly and preserves invocation CWD.
It selects NVIDIA offload when the host provides nvidia-offload. Running install.sh
again refreshes the runtime link. Launching from an unpacked archive uses its flake.

VERSION and release.json identify the package and source; SHA256SUMS verifies all
distributed files. LICENSE contains Apache-2.0; THIRD_PARTY_NOTICES.md and the icon
license retain the embedded assets' notices.

Source and build instructions: https://github.com/cargo-build/roxidian
