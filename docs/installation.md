# Installation

## From Crates.io

Install the latest published release with Cargo:

```bash
cargo install grafatui
```

Grafatui currently requires Rust 1.88 or newer.

## From Source

Clone the repository and install the local checkout:

```bash
git clone https://github.com/fedexist/grafatui.git
cd grafatui
cargo install --path .
```

For development, use `cargo run` instead:

```bash
cargo run -- --prometheus-url http://localhost:9090
```

## Prebuilt Binaries

Prebuilt release assets are published on [GitHub Releases](https://github.com/fedexist/grafatui/releases) for common Linux, macOS, and Windows targets.

## Shell Completions

Grafatui can generate shell completions for Bash, Zsh, Fish, PowerShell, and Elvish.

```bash
# Bash
source <(grafatui completions bash)

# Zsh
source <(grafatui completions zsh)

# Fish
grafatui completions fish | source
```

## Man Page

Generate a man page from the CLI definition:

```bash
grafatui man > grafatui.1
```
