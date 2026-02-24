use std::path::PathBuf;

fn main() {
    // Tauri-generated build configuration (must be called first).
    tauri_build::build();

    let occt_include = occt_include_dir();
    let occt_lib = occt_lib_dir();

    // C++ wrapper compilation — only when OCCT headers and libs are present.
    // A missing OCCT installation is non-fatal: cargo build still succeeds and
    // the geometry module compiles (extern "C" declarations are not resolved
    // unless actually called).
    if occt_include.join("Standard.hxx").exists() && has_occt_lib(&occt_lib) {
        compile_cpp(&occt_include);
        link_occt(&occt_lib);
    } else {
        println!(
            "cargo:warning=OCCT not found (include={}, lib={}); \
             C++ geometry wrapper not compiled. \
             Set OCCT_INCLUDE_DIR and OCCT_LIB_DIR to enable.",
            occt_include.display(),
            occt_lib.display()
        );
    }

    // FFI binding generation via bindgen.
    // cam_geometry.h is pure C (no OCCT headers), so bindgen only needs
    // libclang — it does not require OCCT to be installed.
    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    generate_ffi_bindings(&out_path);

    // Incremental rebuild triggers.
    println!("cargo:rerun-if-changed=cpp/cam_geometry.h");
    println!("cargo:rerun-if-changed=cpp/cam_geometry.cpp");
    println!("cargo:rerun-if-changed=cpp/handle_registry.cpp");
    println!("cargo:rerun-if-env-changed=OCCT_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=OCCT_LIB_DIR");
}

// ── OCCT path resolution ──────────────────────────────────────────────────────

fn occt_include_dir() -> PathBuf {
    std::env::var("OCCT_INCLUDE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_occt_include())
}

fn occt_lib_dir() -> PathBuf {
    std::env::var("OCCT_LIB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_occt_lib())
}

/// Returns true if at least one OCCT toolkit library is present in `dir`.
fn has_occt_lib(dir: &std::path::Path) -> bool {
    // Check for the most fundamental OCCT library in both static and shared forms.
    [
        "libTKernel.a",
        "libTKernel.so",
        "TKernel.lib",
        "TKernel.dll",
    ]
    .iter()
    .any(|name| dir.join(name).exists())
}

#[cfg(target_os = "linux")]
fn default_occt_include() -> PathBuf {
    PathBuf::from("/usr/include/opencascade")
}

#[cfg(target_os = "linux")]
fn default_occt_lib() -> PathBuf {
    // apt installs OCCT into the architecture-specific lib directory.
    let arch_dir = PathBuf::from("/usr/lib/x86_64-linux-gnu");
    if arch_dir.exists() {
        arch_dir
    } else {
        PathBuf::from("/usr/lib")
    }
}

#[cfg(target_os = "macos")]
fn default_occt_include() -> PathBuf {
    // Homebrew uses /opt/homebrew on Apple Silicon, /usr/local on Intel.
    for prefix in ["/opt/homebrew", "/usr/local"] {
        let p = PathBuf::from(prefix).join("include/opencascade");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("/usr/local/include/opencascade")
}

#[cfg(target_os = "macos")]
fn default_occt_lib() -> PathBuf {
    for prefix in ["/opt/homebrew", "/usr/local"] {
        let p = PathBuf::from(prefix).join("lib");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("/usr/local/lib")
}

#[cfg(target_os = "windows")]
fn default_occt_include() -> PathBuf {
    PathBuf::from(r"C:\vcpkg\installed\x64-windows-static\include\opencascade")
}

#[cfg(target_os = "windows")]
fn default_occt_lib() -> PathBuf {
    PathBuf::from(r"C:\vcpkg\installed\x64-windows-static\lib")
}

// Fallback for platforms not explicitly handled above.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn default_occt_include() -> PathBuf {
    PathBuf::from("/usr/include/opencascade")
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn default_occt_lib() -> PathBuf {
    PathBuf::from("/usr/lib")
}

// ── C++ compilation ───────────────────────────────────────────────────────────

fn compile_cpp(occt_include: &std::path::Path) {
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include("cpp")
        .include("cpp/third_party/Clipper2/Clipper2Lib/include")
        .include(occt_include)
        .files([
            "cpp/cam_geometry.cpp",
            "cpp/handle_registry.cpp",
            "cpp/third_party/Clipper2/Clipper2Lib/src/clipper.engine.cpp",
            "cpp/third_party/Clipper2/Clipper2Lib/src/clipper.offset.cpp",
            "cpp/third_party/Clipper2/Clipper2Lib/src/clipper.rectclip.cpp",
        ])
        // Suppress warnings from OCCT and Clipper2 headers we do not control.
        .warnings(false);

    // Windows: CRT linkage must match the vcpkg x64-windows-static OCCT build.
    // vcpkg's x64-windows-static triplet compiles with /MT (static CRT).
    // Rust's MSVC target also defaults to static CRT via the `+crt-static`
    // target feature, so this keeps them in sync.
    #[cfg(target_os = "windows")]
    build.flag("/MT");

    build.compile("cam_geometry");
}

// ── OCCT link directives ──────────────────────────────────────────────────────

const OCCT_LIBS: &[&str] = &[
    "TKernel",
    "TKMath",
    "TKBRep",
    "TKGeomBase",
    "TKGeom2d",
    "TKGeom3d",
    "TKG2d",
    "TKG3d",
    "TKTopAlgo",
    "TKPrim",
    "TKBO",
    "TKShHealing",
    "TKOffset",
    "TKMesh",
    "TKXSBase",
    "TKSTEPBase",
    "TKSTEPAttr",
    "TKSTEP",
    "TKIGES",
    "TKXCAF",
];

// Windows system libraries required by OCCT.
#[cfg(target_os = "windows")]
const WINDOWS_SYSTEM_LIBS: &[&str] = &[
    "Ws2_32", "User32", "Advapi32", "Shell32", "Ole32", "OleAut32", "Gdi32", "Winspool",
];

fn link_occt(occt_lib: &std::path::Path) {
    println!("cargo:rustc-link-search=native={}", occt_lib.display());

    for lib in OCCT_LIBS {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    #[cfg(target_os = "windows")]
    for lib in WINDOWS_SYSTEM_LIBS {
        println!("cargo:rustc-link-lib={lib}");
    }
}

// ── FFI binding generation ────────────────────────────────────────────────────

/// Generate `ffi_generated.rs` from `cpp/cam_geometry.h` using bindgen.
///
/// If bindgen fails (e.g. libclang is not installed), an empty placeholder
/// file is written and a cargo warning is emitted so the build still succeeds.
/// In that case, the `cam_geometry_bindings` cfg flag is *not* set, and any
/// `#[cfg(cam_geometry_bindings)]` test blocks are silently skipped.
///
/// On macOS, set `LIBCLANG_PATH` to the Homebrew LLVM lib directory:
///   export LIBCLANG_PATH=$(brew --prefix llvm)/lib
/// On Windows, set `LIBCLANG_PATH` to the LLVM installation (not the MSVC
/// toolchain — a separate LLVM install is required for bindgen).
fn generate_ffi_bindings(out_path: &std::path::Path) {
    let result = bindgen::Builder::default()
        .header("cpp/cam_geometry.h")
        .allowlist_function("cg_.*")
        .allowlist_type("Cg.*")
        .allowlist_var("CG_.*")
        .generate();

    match result {
        Ok(bindings) => {
            bindings
                .write_to_file(out_path.join("ffi_generated.rs"))
                .expect("failed to write ffi_generated.rs");
            // Signal to the crate that real bindings are present.
            println!("cargo:rustc-cfg=cam_geometry_bindings");
        }
        Err(e) => {
            println!(
                "cargo:warning=bindgen failed ({}); FFI bindings not generated. \
                 Install LLVM/clang and set LIBCLANG_PATH if needed.",
                e
            );
            // Write an empty placeholder so the include!() in ffi.rs compiles.
            std::fs::write(
                out_path.join("ffi_generated.rs"),
                "// FFI bindings not generated — install LLVM/clang.\n",
            )
            .expect("failed to write placeholder ffi_generated.rs");
        }
    }
}
