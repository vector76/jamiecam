# JamieCam Build and Development Setup

## Prerequisites Overview

| Tool | Purpose | Version |
|---|---|---|
| Rust (stable) | Backend language, build toolchain | 1.77+ |
| Node.js | Frontend build runtime | 20 LTS+ |
| pnpm | Frontend package manager | 9+ |
| clang / LLVM | Required by `bindgen` to parse C headers | 16+ |
| OpenCASCADE (OCCT) | Geometry kernel (headers + static libs) | 7.7.x |
| CMake | Builds the C++ wrapper library | 3.20+ |
| Git | Version control | any recent |

---

## Platform Setup

### Linux (Ubuntu 22.04 / Debian 12)

#### System packages

```bash
sudo apt update
sudo apt install -y \
  build-essential curl git cmake pkg-config \
  libssl-dev libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  llvm clang libclang-dev \
  libocct-foundation-dev \
  libocct-modeling-data-dev \
  libocct-modeling-algorithms-dev \
  libocct-data-exchange-dev \
  libocct-ocaf-dev
```

> **Note:** On Ubuntu 20.04, replace `libwebkit2gtk-4.1-dev` with
> `libwebkit2gtk-4.0-dev`. OCCT 7.7.x may need to be built from source on
> older distributions — see the OCCT build note at the bottom of this document.

#### Environment variables

Add to `~/.bashrc` or `~/.zshrc`:

```bash
export OCCT_INCLUDE_DIR=/usr/include/opencascade
export OCCT_LIB_DIR=/usr/lib/x86_64-linux-gnu
export LIBCLANG_PATH=/usr/lib/llvm-16/lib
```

Reload: `source ~/.bashrc`

---

### macOS (13 Ventura or later)

#### Xcode Command Line Tools

```bash
xcode-select --install
```

#### Homebrew packages

```bash
brew install cmake llvm opencascade node pnpm
```

#### Environment variables

Add to `~/.zshrc`:

```bash
# LLVM (for bindgen)
export LLVM_PREFIX=$(brew --prefix llvm)
export LIBCLANG_PATH="$LLVM_PREFIX/lib"
export PATH="$LLVM_PREFIX/bin:$PATH"

# OCCT (Homebrew)
export OCCT_PREFIX=$(brew --prefix opencascade)
export OCCT_INCLUDE_DIR="$OCCT_PREFIX/include/opencascade"
export OCCT_LIB_DIR="$OCCT_PREFIX/lib"
```

Reload: `source ~/.zshrc`

> **Apple Silicon note:** Homebrew installs to `/opt/homebrew` on M-series Macs
> and `/usr/local` on Intel Macs. The `brew --prefix` commands above handle
> both automatically.

---

### Windows 11

All steps use `winget` where possible. Run commands in **PowerShell (Admin)** unless noted.

#### Visual Studio Build Tools

Required by the Rust MSVC toolchain:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools `
  --override "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools `
              --add Microsoft.VisualStudio.Component.Windows11SDK.22621"
```

#### Core tools

```powershell
winget install Rustlang.Rustup
winget install LLVM.LLVM
winget install Kitware.CMake
winget install Git.Git
winget install OpenJS.NodeJS.LTS
```

After Node.js installs:

```powershell
npm install -g pnpm
```

#### OCCT via vcpkg

```powershell
git clone https://github.com/microsoft/vcpkg.git C:\vcpkg
C:\vcpkg\bootstrap-vcpkg.bat
C:\vcpkg\vcpkg install opencascade:x64-windows-static
```

This will take a while — vcpkg builds OCCT from source.

#### Environment variables (System → Advanced → Environment Variables)

| Variable | Value |
|---|---|
| `OCCT_INCLUDE_DIR` | `C:\vcpkg\installed\x64-windows-static\include\opencascade` |
| `OCCT_LIB_DIR` | `C:\vcpkg\installed\x64-windows-static\lib` |
| `LIBCLANG_PATH` | `C:\Program Files\LLVM\lib` |

Restart your terminal after setting these.

> **WebView2:** Ships with Windows 10 (1803+) and Windows 11. No action needed
> for development. For distribution to older machines, the Tauri bundler can
> embed the WebView2 bootstrapper.

---

## Rust Toolchain

After installing `rustup`:

```bash
# Install stable toolchain (if not already default)
rustup toolchain install stable

# Verify
rustc --version   # should show 1.77.0 or later
cargo --version
```

No additional Rust targets are needed for desktop development. Cross-compilation
targets (for CI release builds) are installed by the CI workflow.

---

## Node.js and pnpm

```bash
# Verify Node
node --version    # should show v20.x or later

# Install pnpm if not already installed
npm install -g pnpm
pnpm --version    # should show 9.x or later
```

---

## Repository Setup

```bash
git clone https://github.com/yourorg/jamiecam.git
cd jamiecam
```

### Install frontend dependencies

```bash
pnpm install
```

### Verify the environment

A helper script checks that all required tools and environment variables are present:

```bash
# Linux / macOS
./scripts/check-env.sh

# Windows (PowerShell)
.\scripts\check-env.ps1
```

This script verifies:
- Rust, cargo, Node, pnpm versions
- `OCCT_INCLUDE_DIR` contains `Standard.hxx`
- `OCCT_LIB_DIR` contains `libTKBRep` (Linux/macOS) or `TKBRep.lib` (Windows)
- `LIBCLANG_PATH` contains `libclang` / `libclang.dll`
- `cmake` is on `PATH`

### First build

```bash
# Development build (faster, debug symbols, no optimization)
pnpm tauri dev

# Release build
pnpm tauri build
```

The first build takes significantly longer than subsequent builds because:
1. All Rust crates are compiled from scratch
2. `bindgen` parses the OCCT headers (large)
3. The C++ wrapper is compiled and linked against OCCT

Subsequent builds are incremental. Only changed Rust/C++ files recompile.

---

## Project Structure

```
jamiecam/
│
├── docs/                        architecture documents
│
├── src/                         TypeScript / React frontend
│   ├── main.tsx                 React root
│   ├── api/                     typed invoke() wrappers
│   ├── store/                   Zustand state stores
│   ├── viewport/                Three.js integration
│   └── components/              UI components
│
├── src-tauri/
│   ├── Cargo.toml               Rust dependencies
│   ├── tauri.conf.json          Tauri app configuration
│   ├── build.rs                 compiles C++ wrapper, runs bindgen
│   │
│   ├── src/                     Rust source
│   │   ├── main.rs
│   │   ├── state.rs
│   │   ├── error.rs
│   │   ├── commands/
│   │   ├── geometry/
│   │   ├── toolpath/
│   │   ├── postprocessor/
│   │   └── project/
│   │
│   └── cpp/                     C++ wrapper for OCCT
│       ├── cam_geometry.h       public C API
│       ├── cam_geometry.cpp     OCCT implementation
│       ├── handle_registry.h
│       ├── handle_registry.cpp
│       └── third_party/
│           └── Clipper2/        vendored (git subtree)
│
├── scripts/
│   ├── check-env.sh
│   ├── check-env.ps1
│   └── build-occt.sh            optional: build OCCT from source
│
├── tests/
│   ├── golden/                  golden toolpath and G-code files
│   └── fixtures/                STEP/STL files used in tests
│
├── .github/
│   └── workflows/
│       ├── ci.yml               PR checks
│       └── release.yml          tag-triggered release builds
│
├── package.json
├── vite.config.ts
├── tsconfig.json
├── .gitignore
└── .cargo/
    └── config.toml              Rust build configuration
```

---

## Development Workflow

### Starting the dev server

```bash
pnpm tauri dev
```

This command:
1. Starts the Vite dev server on `http://localhost:1420`
2. Compiles the Rust backend in debug mode
3. Opens a Tauri window pointing at the Vite server
4. Watches for changes:
   - **Frontend changes** → Vite HMR updates the webview instantly (no restart)
   - **Rust changes** → Cargo recompiles, Tauri restarts the window
   - **C++ changes** → `build.rs` detects via `rerun-if-changed`, recompiles the
     C++ wrapper, then Cargo relinks

### Developing the frontend in isolation

The frontend can be developed against a mock IPC layer without the Rust backend:

```bash
pnpm dev       # starts only the Vite dev server, no Tauri
```

A `src/api/mock.ts` module provides stub implementations of all `invoke()` calls
returning hardcoded fixture data. This allows UI development without needing a
compiled Rust binary or an OCCT installation.

Switch between real and mock backends via an env variable:

```bash
VITE_MOCK_API=true pnpm dev
```

### Developing the Rust backend in isolation

```bash
cd src-tauri
cargo test                       # run all Rust tests
cargo test geometry::            # run geometry module tests only
cargo clippy                     # lint
cargo doc --open                 # generate and open API docs
```

### Developing the C++ wrapper in isolation

The C++ wrapper has its own small test harness using `doctest` (header-only,
vendored):

```bash
cd src-tauri/cpp
cmake -B build -DBUILD_TESTS=ON
cmake --build build
./build/cam_geometry_tests
```

### Logging

In dev mode, log output goes to both stderr (visible in the terminal running
`pnpm tauri dev`) and a log file:

```
Linux/macOS: ~/.local/share/jamiecam/logs/jamiecam.log
Windows:     %APPDATA%\jamiecam\logs\jamiecam.log
```

Log level is set via environment variable:

```bash
RUST_LOG=debug pnpm tauri dev          # all debug output
RUST_LOG=jamiecam::geometry=trace pnpm tauri dev   # trace a specific module
```

### Inspecting the WebView

In dev mode, right-click anywhere in the app window → **Inspect Element** opens
the browser DevTools (Chromium DevTools on Windows, WebKit Inspector on macOS).

The Tauri window title shows `[DEV]` in dev mode to distinguish it from release builds.

---

## Environment Variables Reference

| Variable | Required | Description |
|---|---|---|
| `OCCT_INCLUDE_DIR` | Yes | Path to directory containing `Standard.hxx` |
| `OCCT_LIB_DIR` | Yes | Path to directory containing OCCT static/shared libs |
| `LIBCLANG_PATH` | Yes | Path to directory containing `libclang.so` / `libclang.dll` |
| `RUST_LOG` | No | Log filter (e.g. `debug`, `jamiecam=trace`). Default: `info` |
| `VITE_MOCK_API` | No | Set to `true` to use mock IPC stubs in frontend dev |
| `JAMIECAM_OCCT_LOG` | No | Set to `1` to enable OCCT's internal message logging |

### `.cargo/config.toml`

The `.cargo/config.toml` in the repo root holds build configuration that applies
to all developers. It must not contain absolute paths (those go in shell profiles).

```toml
[build]
# Use the faster mold linker on Linux if available (optional, speeds up link step)
# linker = "clang"
# rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[profile.dev]
# Debug builds: skip optimization but keep reasonable build times
opt-level = 0
debug = true

[profile.dev.package."*"]
# Optimize dependencies even in debug builds (makes OCCT FFI calls faster in dev)
opt-level = 2

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = "debuginfo"
```

---

## IDE Setup

### VS Code (recommended)

Install the following extensions:

| Extension | Publisher | Purpose |
|---|---|---|
| `rust-analyzer` | rust-lang | Rust language server, completion, go-to-def |
| `CodeLLDB` | vadimcn | Rust debugger |
| `tauri` | tauri-apps | Tauri command palette, config schema |
| `Even Better TOML` | tamasfe | TOML editing (post-processor configs) |
| `clangd` | LLVM | C++ language server for the wrapper code |
| `ESLint` | Microsoft | TypeScript linting |
| `Prettier` | Prettier | Frontend code formatting |

#### `.vscode/settings.json` (committed to repo)

```json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.check.command": "clippy",
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  },
  "[typescript]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  },
  "[typescriptreact]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  },
  "clangd.arguments": [
    "--compile-commands-dir=${workspaceFolder}/src-tauri/cpp/build"
  ]
}
```

#### `.vscode/launch.json` (committed to repo)

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug Tauri App",
      "cargo": {
        "args": ["build", "--manifest-path", "src-tauri/Cargo.toml"],
        "filter": { "name": "jamiecam", "kind": "bin" }
      },
      "args": [],
      "cwd": "${workspaceFolder}",
      "env": {
        "RUST_LOG": "debug",
        "WEBVIEW_DEVTOOLS": "1"
      }
    }
  ]
}
```

#### Generate `compile_commands.json` for clangd

```bash
cd src-tauri/cpp
cmake -B build -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
```

---

## Running Tests

### All tests

```bash
# Rust tests (unit + integration)
cargo test --manifest-path src-tauri/Cargo.toml

# Frontend typecheck (no emit)
pnpm typecheck

# Frontend lint
pnpm lint

# C++ wrapper tests
cd src-tauri/cpp && cmake -B build -DBUILD_TESTS=ON && cmake --build build && ./build/cam_geometry_tests
```

### Specific test suites

```bash
# Run only toolpath tests
cargo test --manifest-path src-tauri/Cargo.toml toolpath

# Run only post-processor golden tests
cargo test --manifest-path src-tauri/Cargo.toml golden

# Run a single test by name
cargo test --manifest-path src-tauri/Cargo.toml test_pocket_offset_strategy

# Show stdout from tests (useful for debugging)
cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture
```

### Updating golden files

When an intentional algorithm change alters toolpath or G-code output:

```bash
UPDATE_GOLDEN=1 cargo test --manifest-path src-tauri/Cargo.toml golden
```

Review the diff carefully before committing updated golden files.

### Code coverage (optional)

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --manifest-path src-tauri/Cargo.toml --out Html
open tarpaulin-report.html
```

---

## Code Style

### Rust

`rustfmt` is enforced in CI. Format before committing:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
```

`src-tauri/rustfmt.toml`:
```toml
edition = "2021"
max_width = 100
imports_granularity = "Module"
group_imports = "StdExternalCrate"
```

`clippy` is also enforced. Run locally:
```bash
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

Key Clippy rules enforced (in `src-tauri/.clippy.toml` or via attributes):
- No `unwrap()` in `commands/` — must use `?` or explicit match
- No `expect()` with unhelpful messages
- `must_use` on `Result`-returning functions

### TypeScript / React

ESLint + Prettier are enforced in CI:

```bash
pnpm lint          # ESLint check
pnpm format:check  # Prettier check
pnpm format        # Prettier apply
```

`prettier.config.js`:
```js
export default {
  semi: false,
  singleQuote: true,
  printWidth: 100,
  trailingComma: 'all',
}
```

### C++

`clang-format` enforced for `src-tauri/cpp/`:

```bash
clang-format -i src-tauri/cpp/cam_geometry.{h,cpp} src-tauri/cpp/handle_registry.{h,cpp}
```

`.clang-format`:
```yaml
BasedOnStyle: Google
IndentWidth: 4
ColumnLimit: 100
AllowShortFunctionsOnASingleLine: None
```

---

## Git Workflow

### Commit hooks

`cargo fmt`, `cargo clippy`, and `pnpm typecheck` run as pre-commit hooks
via a simple shell script at `.git/hooks/pre-commit`. Install with:

```bash
./scripts/install-hooks.sh
```

### Branch naming

```
feature/<short-description>     new functionality
fix/<short-description>         bug fix
phase/<N>-<name>                phase-level tracking branch
docs/<topic>                    documentation only
```

### Pull request requirements

- CI passes on all 3 platforms
- Golden files updated if output changed (with diff review)
- No `unwrap()` / `expect()` introduced in command handlers
- Architecture docs updated if a documented decision changed

---

## Troubleshooting

### `OCCT_INCLUDE_DIR` not found / `Standard.hxx` not found

`build.rs` cannot locate the OCCT headers.

```bash
# Verify the header exists
ls "$OCCT_INCLUDE_DIR/Standard.hxx"

# Common fixes:
# Linux:   sudo apt install libocct-foundation-dev
# macOS:   brew install opencascade
# Windows: verify vcpkg install completed without errors
```

### `bindgen` fails: `libclang` not found

```
error: failed to run custom build command for `jamiecam`
...
thread 'main' panicked at 'Unable to find libclang'
```

```bash
# Verify LIBCLANG_PATH
ls "$LIBCLANG_PATH/libclang*"         # Linux/macOS
dir "%LIBCLANG_PATH%\libclang*"       # Windows

# Linux fix: sudo apt install libclang-dev
# macOS fix: export LIBCLANG_PATH=$(brew --prefix llvm)/lib
# Windows fix: reinstall LLVM from https://releases.llvm.org/
```

### Link errors: `TKBRep` not found

```
error: linking with `cc` failed
...
ld: library not found: TKBRep
```

```bash
# Verify the library is in OCCT_LIB_DIR
ls "$OCCT_LIB_DIR/"*TKBRep*

# Common cause: OCCT_LIB_DIR points to the wrong directory
# Linux example: should be /usr/lib/x86_64-linux-gnu, not /usr/lib
```

### Windows: MSVC and LLVM conflict

`bindgen` uses LLVM's `libclang` but the MSVC compiler is used to build Rust.
These can coexist but require that `LIBCLANG_PATH` points to LLVM (not MSVC).
Never set `CC` or `CXX` to clang on Windows — let the `cc` crate use MSVC.

### WebView2 development tools don't open

On Windows, right-click → Inspect only works in dev builds. In release builds
the DevTools are disabled. Run `pnpm tauri dev` (not `pnpm tauri build`).

### OCCT version mismatch

If you have multiple OCCT versions installed, `build.rs` may pick up the wrong
one. Set `OCCT_INCLUDE_DIR` and `OCCT_LIB_DIR` explicitly to override any
system defaults.

### Slow first build (> 10 minutes)

Expected. OCCT's headers are large and `bindgen` parses all of them on the first
run. The parsed result is cached by Cargo — subsequent builds are fast unless
`cam_geometry.h` changes. If build times remain slow after the first build, check
that incremental compilation is not disabled (`CARGO_INCREMENTAL=0` should not
be set in your environment).

### macOS: `dyld` errors at runtime

If the Tauri app launches and immediately crashes with a `dyld` library not found error:

```bash
# Check what the binary is looking for
otool -L target/debug/jamiecam

# OCCT homebrew libs may need to be statically linked or @rpath configured
# Ensure build.rs links static libs, not dynamic ones
```

---

## Building OCCT from Source (Optional)

If the platform packages are unavailable or the wrong version, build OCCT directly:

```bash
git clone https://github.com/Open-Cascade-SAS/OCCT.git --branch V7_7_0 --depth 1
cd OCCT
cmake -B build \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_MODULE_Visualization=OFF \
  -DBUILD_MODULE_Draw=OFF \
  -DBUILD_SHARED_LIBS=OFF \
  -DCMAKE_INSTALL_PREFIX=$HOME/occt-install
cmake --build build --parallel $(nproc)
cmake --install build

export OCCT_INCLUDE_DIR=$HOME/occt-install/include/opencascade
export OCCT_LIB_DIR=$HOME/occt-install/lib
```

Disabling `Visualization` and `Draw` modules cuts build time by ~40%.
`BUILD_SHARED_LIBS=OFF` produces static libraries, which simplifies distribution.

---

## CI Environment Reference

The GitHub Actions workflows mirror local setup. For debugging CI failures
that don't reproduce locally, use these exact versions:

| Platform | OS image | OCCT | LLVM |
|---|---|---|---|
| Linux | `ubuntu-22.04` | `apt` packages | `llvm-16` |
| macOS | `macos-13` | `brew install opencascade` | bundled with Xcode + brew llvm |
| Windows | `windows-2022` | vcpkg `7.7.x` | winget LLVM |

The `OCCT_INCLUDE_DIR`, `OCCT_LIB_DIR`, and `LIBCLANG_PATH` are set in each
workflow's `env:` block and can be inspected in `.github/workflows/ci.yml`.

---

*Document status: Draft*
*Related documents: `technology-stack.md`, `development-roadmap.md`*
