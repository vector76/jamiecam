# Windows Build Setup

Complete instructions for building JamieCam on Windows 11. All commands use
**PowerShell (Admin)** unless noted otherwise.

---

## 0. PowerShell Execution Policy

Windows blocks `.ps1` scripts by default. This affects both the `check-env.ps1`
script in this repo and npm global package shims (e.g. `pnpm.ps1`). Fix it once
before doing anything else:

```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

`RemoteSigned` allows locally created scripts to run freely, and requires a
trusted signature only for scripts downloaded from the internet. This is the
standard developer setting on Windows.

---

## 1. Install Prerequisites

### Visual Studio Build Tools

The Rust MSVC toolchain requires the MSVC C++ compiler and Windows SDK:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools `
  --override "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools `
              --add Microsoft.VisualStudio.Component.Windows11SDK.22621"
```

### Core tools

```powershell
winget install Rustlang.Rustup
winget install LLVM.LLVM
winget install Kitware.CMake
winget install Git.Git
winget install OpenJS.NodeJS.LTS
```

After Node.js installs, open a new terminal and install pnpm:

```powershell
npm install -g pnpm
```

Restart your terminal after all installs so PATH changes take effect.

---

## 2. Install OCCT via vcpkg

OCCT is installed as a static library using vcpkg. This step builds OCCT from
source — **expect 20–40 minutes** on first run.

```powershell
git clone https://github.com/microsoft/vcpkg.git C:\vcpkg --depth 1
C:\vcpkg\bootstrap-vcpkg.bat
C:\vcpkg\vcpkg install opencascade:x64-windows-static --vcpkg-root C:\vcpkg
```

The `x64-windows-static` triplet is required. It builds OCCT with the static
CRT (`/MT`), which must match the Rust build configuration.

---

## 3. Set Environment Variables

These must be set as **System** environment variables (not just user-session) so
that both interactive terminals and editor processes pick them up.

Open **System Properties → Advanced → Environment Variables** and add:

| Variable | Value |
|---|---|
| `OCCT_INCLUDE_DIR` | `C:\vcpkg\installed\x64-windows-static\include\opencascade` |
| `OCCT_LIB_DIR` | `C:\vcpkg\installed\x64-windows-static\lib` |
| `LIBCLANG_PATH` | `C:\Program Files\LLVM\lib` |

Or set them via PowerShell (applies immediately to future sessions):

```powershell
[System.Environment]::SetEnvironmentVariable("OCCT_INCLUDE_DIR",
  "C:\vcpkg\installed\x64-windows-static\include\opencascade", "Machine")

[System.Environment]::SetEnvironmentVariable("OCCT_LIB_DIR",
  "C:\vcpkg\installed\x64-windows-static\lib", "Machine")

[System.Environment]::SetEnvironmentVariable("LIBCLANG_PATH",
  "C:\Program Files\LLVM\lib", "Machine")
```

**Restart your terminal** after setting these before continuing.

---

## 4. Verify the Environment

A check script confirms all required tools and variables are present:

```powershell
.\scripts\check-env.ps1
```

It verifies:
- `rustc`, `cargo`, `node`, `pnpm`, `cmake` versions
- `OCCT_INCLUDE_DIR` contains `Standard.hxx`
- `OCCT_LIB_DIR` contains `TKBRep.lib`
- `LIBCLANG_PATH` contains `libclang.dll`

Fix any failures it reports before proceeding.

---

## 5. Clone and Build

```powershell
git clone <repo-url> jamiecam
cd jamiecam
pnpm install
```

### Development build (recommended for day-to-day work)

```powershell
pnpm tauri dev
```

This starts the Vite dev server, compiles the Rust backend in debug mode, and
opens the app window. Hot-reload is active for frontend changes; Rust/C++ changes
trigger a recompile and window restart automatically.

### Release build

```powershell
pnpm tauri build
```

Produces a `.exe` installer and `.msi` package under
`src-tauri/target/release/bundle/`.

---

## 6. First Build Notes

The first build takes significantly longer than subsequent ones:

- `bindgen` parses the large OCCT headers to generate Rust FFI bindings
- All Rust crates compile from scratch
- The C++ wrapper compiles and links against OCCT static libraries

Subsequent builds are incremental. Only changed Rust or C++ files recompile.
The bindgen output is cached and only regenerates if `cam_geometry.h` changes.

---

## 7. Running Tests

### Rust tests

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Run a subset by name pattern:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml geometry
cargo test --manifest-path src-tauri/Cargo.toml golden
```

### C++ wrapper tests

```powershell
cmake -B src-tauri/cpp/build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTS=ON src-tauri/cpp
cmake --build src-tauri/cpp/build --config Release
ctest --test-dir src-tauri/cpp/build -C Release --output-on-failure
```

### Frontend

```powershell
pnpm typecheck
pnpm lint
pnpm test
```

---

## 8. Development Workflow

### Frontend only (no Rust required)

The frontend can run against a mock backend without a compiled Rust binary or
an OCCT installation:

```powershell
$env:VITE_MOCK_API = "true"
pnpm dev
```

This starts the Vite dev server on `http://localhost:1420` with stub IPC calls.

### Logging

Log output goes to the terminal running `pnpm tauri dev` and to:

```
%APPDATA%\jamiecam\logs\jamiecam.log
```

Control log verbosity:

```powershell
$env:RUST_LOG = "debug"
pnpm tauri dev

$env:RUST_LOG = "jamiecam::geometry=trace"
pnpm tauri dev
```

### Inspecting the WebView

In dev mode, right-click anywhere in the app window → **Inspect Element** opens
Chromium DevTools. This is only available in dev builds (`pnpm tauri dev`), not
release builds.

---

## 9. Troubleshooting

### `Standard.hxx` not found / OCCT headers missing

`build.rs` cannot locate the OCCT headers. Verify the path is correct:

```powershell
dir "$env:OCCT_INCLUDE_DIR\Standard.hxx"
```

If the file is missing, the vcpkg install may have failed or the variable points
to the wrong directory. Re-run:

```powershell
C:\vcpkg\vcpkg install opencascade:x64-windows-static --vcpkg-root C:\vcpkg
```

### `bindgen` fails: `libclang` not found

```
thread 'main' panicked at 'Unable to find libclang'
```

Verify:

```powershell
dir "$env:LIBCLANG_PATH\libclang*"
```

The default LLVM install path is `C:\Program Files\LLVM`. `LIBCLANG_PATH` must
point to **LLVM's** `lib` directory — not OCCT's. A common mistake is setting
it to `C:\vcpkg\installed\x64-windows-static\lib` (which is `OCCT_LIB_DIR`).
The correct value is `C:\Program Files\LLVM\lib`.

Reinstall LLVM via winget if the directory is missing:

```powershell
winget install LLVM.LLVM
```

### Link errors: `TKBRep` not found

```
error: linking with `link.exe` failed
...
LINK : fatal error LNK1181: cannot open input file 'TKBRep.lib'
```

Verify:

```powershell
dir "$env:OCCT_LIB_DIR\TKBRep.lib"
```

Common cause: `OCCT_LIB_DIR` is set to the wrong directory, or the
`x64-windows-static` triplet was not used when installing via vcpkg.

### MSVC and LLVM coexistence

`bindgen` uses LLVM's `libclang` to parse headers, but the Rust build uses the
MSVC compiler (`cl.exe`) to compile C++. This is intentional and expected.

Do **not** set `CC` or `CXX` to clang on Windows — let the `cc` crate pick up
MSVC automatically. Only `LIBCLANG_PATH` needs to point at LLVM.

### Environment variables not picked up by editor/IDE

If VS Code or another editor launches before the environment variables were set,
it will not see them. Restart the editor after setting the system variables.
Verify from a fresh terminal that the variables are visible:

```powershell
echo $env:OCCT_INCLUDE_DIR
echo $env:OCCT_LIB_DIR
echo $env:LIBCLANG_PATH
```

### Slow incremental builds

If builds remain slow after the first run, check that incremental compilation is
not disabled:

```powershell
echo $env:CARGO_INCREMENTAL
```

This should be empty or `1`. If it is `0`, unset it:

```powershell
Remove-Item Env:CARGO_INCREMENTAL
```

---

*Related documents: `build-and-dev-setup.md` (all platforms), `technology-stack.md`*
