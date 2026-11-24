# Installation Guide

This guide covers installing `pkl-rs` as standalone CLI tools (`pkl`, `pkldoc`), a Rust library (`crates/pkl`), a Node.js native addon (`@openedges/pkl-napi`), or a WebAssembly package (`@openedges/pkl-wasm`), as well as building from source.

---

## 1. Homebrew (macOS & Linux)

Install both `pkl` and `pkldoc` (plus bundled `bash`/`zsh`/`fish` shell completions and `LICENSE`/`NOTICE`/`THIRD-PARTY-NOTICES.md`) from the `openedges/homebrew-tap` tap:

```sh
brew install openedges/tap/pkl-rs
pkl --version
pkldoc --version
```

*(Conflicts with Homebrew's `pkl` formula because both provide a `pkl` executable.)*

---

## 2. Prebuilt CLI Binaries (`pkl` and `pkldoc`)

Prebuilt archives are built by [.github/workflows/release.yml](../.github/workflows/release.yml) and published to [GitHub Releases](https://github.com/openedges/pkl-rs/releases). Every archive bundles the executable alongside `LICENSE`, `NOTICE`, and `THIRD-PARTY-NOTICES.md`.

### Supported Platforms

| OS | Architecture | Archive Name Pattern | Runtime Requirement |
| --- | --- | --- | --- |
| **Linux (glibc)** | `x86_64` | `pkl-linux-x64-gnu.tar.gz`, `pkldoc-linux-x64-gnu.tar.gz` | glibc `>= 2.28` (Debian 10+, Ubuntu 20.04+, RHEL/Rocky 8+) |
| **Linux (glibc)** | `aarch64` | `pkl-linux-arm64-gnu.tar.gz`, `pkldoc-linux-arm64-gnu.tar.gz` | glibc `>= 2.28` |
| **Linux (static musl)** | `x86_64` | `pkl-linux-x64-musl.tar.gz`, `pkldoc-linux-x64-musl.tar.gz` | None (fully static binary; Alpine / scratch containers) |
| **Linux (static musl)** | `aarch64` | `pkl-linux-arm64-musl.tar.gz`, `pkldoc-linux-arm64-musl.tar.gz` | None (fully static binary) |
| **macOS** | Apple Silicon (`arm64`) | `pkl-darwin-arm64.tar.gz`, `pkldoc-darwin-arm64.tar.gz` | macOS 12+ |
| **macOS** | Intel (`x86_64`) | `pkl-darwin-x64.tar.gz`, `pkldoc-darwin-x64.tar.gz` | macOS 12+ |
| **Windows** | `x86_64` | `pkl-win32-x64.zip`, `pkldoc-win32-x64.zip` | Windows 10/11 (`x64`) |

### Download and Verify Checksums

#### macOS / Linux

```sh
VERSION=v0.32.0-rc.2
ARCHIVE=pkl-darwin-arm64.tar.gz   # Adjust for your platform above

curl -fSLO "https://github.com/openedges/pkl-rs/releases/download/${VERSION}/${ARCHIVE}"
curl -fSLO "https://github.com/openedges/pkl-rs/releases/download/${VERSION}/SHA256SUMS.txt"

# Verify SHA-256 (macOS: shasum -a 256; Linux: sha256sum)
sha256sum --check --ignore-missing SHA256SUMS.txt

tar -xzf "${ARCHIVE}"
install -m 755 pkl /usr/local/bin/pkl
pkl --version
```

#### Windows (PowerShell)

```powershell
$Version = "v0.32.0-rc.2"
$Archive = "pkl-win32-x64.zip"

Invoke-WebRequest "https://github.com/openedges/pkl-rs/releases/download/$Version/$Archive" -OutFile $Archive
Invoke-WebRequest "https://github.com/openedges/pkl-rs/releases/download/$Version/SHA256SUMS.txt" -OutFile SHA256SUMS.txt

$Expected = (Select-String -Path SHA256SUMS.txt -Pattern $Archive).Line.Split()[0]
$Actual = (Get-FileHash $Archive -Algorithm SHA256).Hash.ToLower()
if ($Actual -ne $Expected) { throw "Checksum mismatch: $Actual != $Expected" }

Expand-Archive $Archive -DestinationPath "$env:LOCALAPPDATA\pkl-rs" -Force
& "$env:LOCALAPPDATA\pkl-rs\pkl.exe" --version
```

### Shell Completions

`pkl` emits built-in completion scripts for `bash`, `zsh`, and `fish`:

```sh
# Bash
pkl shell-completion bash > ~/.local/share/bash-completion/completions/pkl

# Zsh (place in a directory on your $fpath)
pkl shell-completion zsh > "${HOME}/.zfunc/_pkl"

# Fish
pkl shell-completion fish > ~/.config/fish/completions/pkl.fish
```

---

## 3. Node.js Native Package (`@openedges/pkl-napi`)

- **Node.js requirement**: `>= 18.0.0` (N-API v8)
- **Bundled prebuilt addons (7 platforms)**: `darwin-arm64`, `darwin-x64`, `linux-x64-gnu` (glibc `>= 2.28`), `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `win32-x64-msvc`.
- **Module formats**: Dual ESM (`import`) and CommonJS (`require`) with bundled TypeScript declarations (`dist/index.d.ts`).

```sh
# Release candidates use the `next` dist-tag (GA uses `npm install @openedges/pkl-napi`)
npm install @openedges/pkl-napi@next
```

To build and test locally from a source checkout:

```sh
node packages/pkl-napi/scripts/build.mjs
node packages/pkl-napi/test/run.mjs
node packages/pkl-napi/test/pack-install.mjs
```

See [packages/pkl-napi/README.md](../packages/pkl-napi/README.md) for API usage and examples.

---

## 4. WebAssembly Package (`@openedges/pkl-wasm`)

- **Browser support**: Modern evergreen browsers supporting WebAssembly + ES Modules (Chromium, Firefox, WebKit/Safari).
- **Node.js requirement**: `>= 20.16.0` (ESM).
- **Bundle budget**: `pkl_wasm_bg.wasm` stays within `<= 2.50 MiB` (`2,621,440 B`) gzip-9 compressed.

```sh
# Release candidates use the `next` dist-tag (GA uses `npm install @openedges/pkl-wasm`)
npm install @openedges/pkl-wasm@next
```

To build and test locally from a source checkout (requires `wasm32-unknown-unknown` target and pinned `wasm-bindgen-cli 0.2.108`):

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.108 --locked --root .cache/m1-tools
node packages/pkl-wasm/scripts/build.mjs
node --test packages/pkl-wasm/test/wasm.test.mjs
```

See [packages/pkl-wasm/README.md](../packages/pkl-wasm/README.md) for browser/Node usage and explicit Wasm capability boundaries.

---

## 5. Rust Crates (`crates.io`)

The public Rust evaluator API crate (`crates/pkl`) is published on crates.io as **`pkl-rs`** while exporting its library module as `pkl` (`use pkl::...;`), alongside the modular workspace crates (`pkl-runtime`, `pkl-formatter`, `pkl-host-native`, `pkl-repl`, `pkl-server`, `pkl-doc`, `pkl-cli`):

```toml
[dependencies]
pkl-rs = "0.32.0-rc.2"
```

Or install the CLI tools directly via Cargo:

```sh
cargo install pkl-cli --version 0.32.0-rc.2
cargo install pkl-doc --version 0.32.0-rc.2
```

Git dependency consumption also works directly with zero `[patch.crates-io]` overrides:

```toml
[dependencies]
pkl-rs = { git = "https://github.com/openedges/pkl-rs.git", tag = "v0.32.0-rc.2" }
```

- **Minimum Supported Rust Version (MSRV)**: Rust `1.98` (workspace pinned to `1.98.1` in [rust-toolchain.toml](../rust-toolchain.toml), Rust 2024 edition).

---

## 6. Building from Source

### Prerequisites

- Git
- [rustup](https://rustup.rs/) (automatically selects Rust `1.98.1` from `rust-toolchain.toml`)
- Node.js (`>= 20.16.0`) — only required if building `@openedges/pkl-wasm` or `@openedges/pkl-napi`
- Python `3.10+` — only required for contributor/release verification harnesses (`tools/local_verify.py`)

### Build CLI Binaries Only (No Submodules Required)

```sh
git -c core.autocrlf=false clone https://github.com/openedges/pkl-rs.git
cd pkl-rs
cargo build --release --locked -p pkl-cli --bin pkl -p pkl-doc --bin pkldoc
./target/release/pkl --version
./target/release/pkldoc --version
```

### Full Contributor Clone (Including Reference Submodules for Testing)

Preserve upstream fixture line endings (`core.autocrlf=false`) so raw-byte compatibility checks stay exact on all platforms including Windows:

```sh
git -c core.autocrlf=false clone --recurse-submodules https://github.com/openedges/pkl-rs.git
cd pkl-rs
git config --local core.autocrlf false
git submodule foreach --recursive 'git config --local core.autocrlf false'
cargo xtask verify
```

For full local test and verification commands, see [TESTING.md](../TESTING.md) and [docs/ci.md](ci.md).
