default:
    @just --list

build:
    cargo build -j 2

build-release:
    cargo build --release -j 2

check:
    cargo check --workspace -j 2

test:
    cargo test -j 2

run *args:
    cargo run -p roxidian -j 2 -- {{args}}

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets -j 2 -- -D warnings

install:
    mkdir -p "$HOME/.local/bin"
    ln -sfn "{{justfile_directory()}}/scripts/rox" "$HOME/.local/bin/rox"

gate: fmt-check clippy test
