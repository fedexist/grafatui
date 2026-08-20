# Installation

## Installer Script

Install the latest prebuilt release without requiring Rust:

```bash
bash -o pipefail -c 'curl --proto =https --tlsv1.2 -LsSf https://raw.githubusercontent.com/fedexist/grafatui/main/install.sh | bash'
```

With `wget`:

```bash
bash -o pipefail -c 'wget -O- https://raw.githubusercontent.com/fedexist/grafatui/main/install.sh | bash'
```

The script supports Linux and macOS on x86_64 and ARM64. It installs to
`$HOME/.local/bin` and never invokes `sudo`. Make sure that directory is on
your `PATH`.

Set `GRAFATUI_INSTALL_DIR` to choose another destination:

```bash
bash -o pipefail -c 'curl --proto =https --tlsv1.2 -LsSf https://raw.githubusercontent.com/fedexist/grafatui/main/install.sh | GRAFATUI_INSTALL_DIR=/custom/bin bash'
```

Set `GRAFATUI_VERSION` to install a specific release. The leading `v` is
optional:

```bash
bash -o pipefail -c 'curl --proto =https --tlsv1.2 -LsSf https://raw.githubusercontent.com/fedexist/grafatui/main/install.sh | GRAFATUI_VERSION=v0.1.11 bash'
```

Every release download is verified against its published SHA-256 checksum
manifest. Installation stops if the manifest is unavailable or verification
fails.

Reviewing downloaded scripts before running them is recommended:

```bash
curl --proto '=https' --tlsv1.2 -LsSf -o install.sh https://raw.githubusercontent.com/fedexist/grafatui/main/install.sh
less install.sh
bash install.sh
```

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
