# Knowlix

Local-first desktop knowledge search app built with Tauri v2, Rust, and React.

## Prerequisites

### All Platforms

Install Rust via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
rustup default stable
```

Install Node.js (v18+) and npm.

### Linux (Debian/Ubuntu)

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### Linux (Arch)

```bash
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl \
  appmenu-gtk-module libappindicator-gtk3 librsvg xdotool
```

### Linux (Fedora)

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel
sudo dnf group install "c-development"
```

### macOS

```bash
xcode-select --install
```

### Windows

1. Install [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) — select **Desktop development with C++**
2. WebView2 Runtime — pre-installed on Windows 10 v1803+, otherwise [download from Microsoft](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

---

## Development

Install frontend dependencies:

```bash
cd apps/desktop
npm install
```

Run dev mode (starts Vite dev server + Tauri shell together):

```bash
npm run tauri dev
```

> **Note:** First run compiles all Rust crates — may take 5–15 minutes. Subsequent runs are much faster.

Hot reload is active for both frontend (React/Vite) and backend (Rust recompiles on save).

Open DevTools: right-click anywhere → **Inspect**, or `Ctrl+Shift+I` (Linux/Windows) / `Cmd+Option+I` (macOS).

### Logs

Rust backend logs at `INFO` level by default. Logs are printed to the terminal running `npm run tauri dev`.

```
INFO [reindex] project=<id> folders=["/your/folder"]
INFO [reindex] walk folder=/your/folder found=5 files
INFO [reindex] indexed /your/folder/notes.md
```

To change log level, set `RUST_LOG` before running:

```bash
RUST_LOG=debug npm run tauri dev
```

### Type-check Rust without running the app

```bash
# Check all workspace crates
cargo check --workspace

# Check a specific crate
cargo check -p knowlix-indexer
```

---

## Testing

All tests are Rust unit/integration tests in each crate under `core/`.

### Run all tests

```bash
cargo test --workspace
```

### Run tests for a specific crate

```bash
cargo test -p knowlix-indexer
cargo test -p knowlix-storage
cargo test -p knowlix-project
cargo test -p knowlix-search
```

### Show println / log output

```bash
cargo test --workspace -- --nocapture
```

### Run a single test by name

```bash
cargo test -p knowlix-indexer test_reindex_project_discovers_new_files -- --nocapture
```

### What each crate tests

| Crate | Tests cover |
|---|---|
| `knowlix-indexer` | `walk_folder` filtering, recursive dir walking, `reindex_project` discovers new files, skips unchanged files, `chunk_text` splitting and overlap, file type and language detection, SHA256 hashing |
| `knowlix-storage` | Project CRUD, duplicate name rejection, file entry upsert/delete, chunk insert/delete, FTS snippet truncation |
| `knowlix-project` | Project creation, empty name validation, duplicate name rejection, update, list, delete |
| `knowlix-search` | Keyword FTS search returns ranked results |

---

## Build

Build a production bundle for the current platform:

```bash
cd apps/desktop
npm run tauri build
```

Bundles are output to `apps/desktop/src-tauri/target/release/bundle/`.

### Platform output formats

| Platform | Formats |
|---|---|
| Linux | `.deb`, `.AppImage`, `.rpm` |
| macOS | `.app`, `.dmg` |
| Windows | `-setup.exe` (NSIS), `.msi` |

### Build specific formats only

```bash
# Compile without packaging
npm run tauri build -- --no-bundle

# Package specific formats
npm run tauri build -- --bundles deb,appimage     # Linux
npm run tauri build -- --bundles app,dmg          # macOS
npm run tauri build -- --bundles nsis             # Windows (must run on Windows)
```

### Cross-compile Windows `.exe` from Linux

> **Note:** This produces a raw `.exe` binary only — no NSIS installer. For a full Windows installer, use a Windows machine or GitHub Actions.

```bash
# Install cross-compile toolchain (Debian/Ubuntu)
sudo apt install gcc-mingw-w64-x86-64

# Add Windows Rust target
rustup target add x86_64-pc-windows-gnu

# Build
npm run tauri build -- --target x86_64-pc-windows-gnu
```

Output: `src-tauri/target/x86_64-pc-windows-gnu/release/knowlix-desktop.exe`

### macOS architecture targets

```bash
# Apple Silicon
npm run tauri build -- --target aarch64-apple-darwin

# Intel
npm run tauri build -- --target x86_64-apple-darwin
```

---

## CI/CD (GitHub Actions)

For cross-platform releases, use native runners per platform. Example matrix:

```yaml
strategy:
  matrix:
    include:
      - platform: ubuntu-22.04
        args: ''
      - platform: macos-latest
        args: '--target aarch64-apple-darwin'
      - platform: windows-latest
        args: ''

steps:
  - if: matrix.platform == 'ubuntu-22.04'
    run: |
      sudo apt update
      sudo apt install libwebkit2gtk-4.1-dev build-essential libxdo-dev \
        libssl-dev libayatana-appindicator3-dev librsvg2-dev

  - if: contains(matrix.args, 'apple')
    run: rustup target add aarch64-apple-darwin

  - uses: tauri-apps/tauri-action@v0
    env:
      GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    with:
      tagName: app-v__VERSION__
      args: ${{ matrix.args }}
```

---

## Project Structure

```
knowlix/
├── apps/
│   └── desktop/          # Tauri + React frontend
│       ├── src/          # React app
│       └── src-tauri/    # Rust backend
├── core/                 # Shared Rust workspace crates
│   ├── common/
│   ├── project/
│   ├── indexer/
│   ├── search/
│   ├── watcher/
│   ├── storage/
│   ├── viewer/
│   ├── wiki/
│   └── ai_agent/
└── Cargo.toml            # Workspace root
```
