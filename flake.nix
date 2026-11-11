{
  description = "roxidian — build and agent tooling environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Runtime libraries for a Rust GUI application: Wayland/X11 windowing,
        # GPU (Vulkan/GL), fonts, audio, TLS and the *-sys crates.
        libraries = with pkgs; [
          glibc
          stdenv.cc.cc.lib
          alsa-lib
          fontconfig
          freetype
          libGL
          libgit2
          libxkbcommon
          libX11
          libXcursor
          libXi
          libXinerama
          libXrandr
          libxcb
          openssl
          pipewire
          sqlite
          udev
          vulkan-loader
          wayland
          zlib
          zstd
        ];
        libraryPath = pkgs.lib.makeLibraryPath libraries;
      in
      {
        packages.runtime = pkgs.writeTextDir "library-path" (libraryPath + "\n");

        devShells.default = pkgs.mkShell {
          name = "roxidian";

          # The Rust toolchain is the operator's rustup install, never a nixpkgs
          # substitute: `cargo`/`rustc` must be the same build as `clippy`.
          nativeBuildInputs = with pkgs; [ pkg-config cmake just python3 jq uv ];

          buildInputs = libraries;

          LD_LIBRARY_PATH = libraryPath;
          PKG_CONFIG_PATH = pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" libraries;

          shellHook = ''
            echo "[roxidian] devShell ready - rustup toolchain, GUI libraries, just"
          '';
        };

        devShells.runtime = pkgs.mkShellNoCC {
          name = "roxidian-runtime";
          packages = libraries;
          LD_LIBRARY_PATH = libraryPath;
        };

        formatter = pkgs.nixfmt-rfc-style;
      });
}
