#!/usr/bin/env python3
"""Package the verified native release with an independent Nix runtime."""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tomllib


ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-linux-nixos"


def run(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def source_revision(explicit):
    if explicit is not None:
        revision = explicit
    elif (ROOT / ".jj").is_dir() and shutil.which("jj"):
        revision = run("jj", "log", "-r", "latest((@ | @-) ~ empty())", "--no-graph", "-T", "commit_id")
    elif (ROOT / ".git").exists() and shutil.which("git"):
        subprocess.run(["git", "diff", "--exit-code", "HEAD"], cwd=ROOT, check=True)
        revision = run("git", "rev-parse", "HEAD")
    else:
        raise ValueError("source revision is required; use --revision with the source commit")
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("source revision must be a full commit ID")
    return revision


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def package(revision):
    if os.uname().machine != "x86_64" or os.uname().sysname != "Linux":
        raise ValueError("this distribution currently supports x86_64 Linux")
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    version = manifest["workspace"]["package"]["version"]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("distribution version must be a stable semantic version")
    binary = ROOT / "target/release/rox"
    if run(str(binary), "--version") != f"rox {version}":
        raise ValueError("compiled binary version differs from Cargo.toml")
    name = f"roxidian-{version}-{TARGET}"
    dist = ROOT / "target/dist" / revision
    bundle = dist / name
    archive = dist / f"{name}.tar.gz"
    if bundle.exists():
        existing = json.loads((bundle / "release.json").read_text())
        if (existing["source_revision"], existing["version"], existing["build_binary_sha256"]) != (revision, version, digest(binary)):
            raise FileExistsError(f"{bundle}: a different build already exists; preserving it")
        subprocess.run(["sha256sum", "--check", "SHA256SUMS"], cwd=bundle, check=True)
        subprocess.run(["sha256sum", "--check", "SHA256SUMS"], cwd=dist, check=True)
        print(archive)
        return bundle
    bundle.mkdir(parents=True)
    files = {
        binary: "bin/rox", ROOT / "scripts/rox": "scripts/rox",
        ROOT / "flake.nix": "runtime/flake.nix", ROOT / "flake.lock": "runtime/flake.lock",
        ROOT / "packaging/install.sh": "install.sh", ROOT / "packaging/README.md": "README.md",
        ROOT / "LICENSE": "LICENSE", ROOT / "THIRD_PARTY_NOTICES.md": "THIRD_PARTY_NOTICES.md",
        ROOT / "assets/file-icons/LICENSE": "assets/file-icons/LICENSE",
    }
    for source, relative in files.items():
        destination = bundle / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        destination.chmod(0o755 if relative in ("bin/rox", "scripts/rox", "install.sh") else 0o644)
    subprocess.run(["strip", "--strip-unneeded", str(bundle / "bin/rox")], check=True)
    (bundle / "VERSION").write_text(version + "\n")
    info = {"version": version, "target": TARGET, "libc": "glibc", "source_revision": revision,
            "toolchain": run("rustc", "--version"), "binary_sha256": digest(bundle / "bin/rox"),
            "build_binary_sha256": digest(binary),
            "cargo_lock_sha256": digest(ROOT / "Cargo.lock")}
    (bundle / "release.json").write_text(json.dumps(info, indent=2) + "\n")
    entries = sorted(path for path in bundle.rglob("*") if path.is_file())
    (bundle / "SHA256SUMS").write_text("".join(f"{digest(path)}  {path.relative_to(bundle)}\n" for path in entries))
    with archive.open("wb") as raw, gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as tar:
            def normalize(member):
                member.uid = member.gid = member.mtime = 0
                member.uname = member.gname = ""
                return member
            tar.add(bundle, arcname=name, filter=normalize)
    (dist / "SHA256SUMS").write_text(f"{digest(archive)}  {archive.name}\n")
    print(archive)
    return bundle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", help="source commit, required outside a versioned checkout")
    parser.add_argument("--install", action="store_true", help="install the packaged release under ~/.local")
    args = parser.parse_args()
    bundle = package(source_revision(args.revision))
    if args.install:
        subprocess.run([str(bundle / "install.sh")], check=True)


if __name__ == "__main__":
    main()
