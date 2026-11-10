# roxidian — SynthWave reader

Markdown paragraphs follow the available window width.
Ordinary line endings in the source flow together,
while explicit hard breaks remain intentional.

## Shell commands, flags and values

```bash
pacman -S ssh pacman curl
LOG_LEVEL=debug curl --output report.json --retry 3 https://example.com
ssh root@host 'bash -s' <<'OUTER'
set -e
pacman -S --noconfirm --needed fish starship atuin \
  tcpdump mtr iperf3 bind-tools openbsd-netcat socat nload iftop lsof jq nmap \
  curl wget git vim htop unzip rsync openssl which \
  ufw fail2ban python-systemd
fish --version; starship --version | head -1; atuin --version
# Commands glow; arguments stay upright; values use italics.
OUTER
```

## Your notes, in focus.

A quiet place to read Markdown, explore a workspace and inspect the **original source**.

> Follow the document. Keep your context.

## One document. Two perspectives.

Switch between **Preview**, *Source* and a side-by-side split. Inline `code` stays distinct;
~~discarded ideas~~ can stay in the text.

```rust
fn main() {
    println!("Hello, roxidian 🦀");
}
```

## Explore without editing

- Expand folders in the workspace sidebar.
- Right-click a folder and choose **Open as workspace**.
- Use **↑ Parent** to browse the containing directory.

| View | Purpose |
| --- | --- |
| Preview | Read the rendered document |
| Source | Inspect the original Markdown |
| Split | Keep both perspectives visible |

- [x] Readonly documents
- [x] SynthWave dark and Obsidian-inspired light
- [ ] Your next idea

Read the [project introduction](../../README.md), or return to [the top](#your-notes-in-focus).

---

No document writes. Your notes stay yours.
