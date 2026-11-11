#!/usr/bin/env bash
set -euo pipefail
bundle="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"
read -r version < "$bundle/VERSION"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "roxidian: invalid distribution version" >&2
    exit 1
fi
root="$(realpath -m -- "${ROXIDIAN_INSTALL_ROOT:-$HOME/.local/roxidian}")"
bin_dir="$(realpath -m -- "${ROXIDIAN_BIN_DIR:-$HOME/.local/bin}")"
destination="$root/$version"
mkdir -p "$root" "$bin_dir"
exec 9>"$root/.install.lock"
flock 9
if [[ -L "$destination" ]]; then
    echo "roxidian: version directory is a symlink; refusing to replace it" >&2
    exit 1
fi
if [[ -e "$root/current" && ! -L "$root/current" ]] || [[ -e "$bin_dir/rox" && ! -L "$bin_dir/rox" ]]; then
    echo "roxidian: current or rox is not a managed symlink; refusing to replace it" >&2
    exit 1
fi
(cd "$bundle" && sha256sum --check SHA256SUMS)
if [[ -e "$destination" ]]; then
    if ! cmp --silent "$bundle/SHA256SUMS" "$destination/SHA256SUMS"; then
        echo "roxidian: $destination contains a different build; refusing to overwrite it" >&2
        exit 1
    fi
    (cd "$destination" && sha256sum --check SHA256SUMS)
else
    staging="$(mktemp -d "$root/.install-$version.XXXXXXXX")"
    trap 'rm -r -- "$staging"' EXIT
    cp -a "$bundle/." "$staging/"
    (cd "$staging" && sha256sum --check SHA256SUMS)
    mv -T -- "$staging" "$destination"
    trap - EXIT
fi
(cd "$destination/runtime" && nix build --no-write-lock-file --out-link "$destination/.runtime" 'path:.#runtime')
if [[ ! -s "$destination/.runtime/library-path" ]]; then
    echo "roxidian: prepared runtime has no library path" >&2
    exit 1
fi
installed_version="$("$destination/scripts/rox" --version)"
if [[ "$installed_version" != "rox $version" ]]; then
    echo "roxidian: installed executable reports an unexpected version: $installed_version" >&2
    exit 1
fi

link_stage=''
trap 'if [[ -n "$link_stage" ]]; then rm -r -- "$link_stage"; fi' EXIT
activate() {
    local target="$1" entry="$2"
    link_stage="$(mktemp -d "$(dirname "$entry")/.rox-link.XXXXXXXX")"
    ln -s -- "$target" "$link_stage/link"
    mv -Tf -- "$link_stage/link" "$entry"
    rmdir -- "$link_stage"
    link_stage=''
}
activate "$version" "$root/current"
activate "$root/current/scripts/rox" "$bin_dir/rox"
trap - EXIT
printf 'Installed roxidian %s: %s\n' "$version" "$destination"
