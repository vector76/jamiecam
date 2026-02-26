use std::path::PathBuf;

fn main() {
    // Tauri-generated build configuration (must be called first).
    tauri_build::build();

    let occt_include = occt_include_dir();
    let occt_lib = occt_lib_dir();

    // C++ wrapper compilation — only when OCCT headers and libs are present.
    // A missing OCCT installation is non-fatal: cargo build still succeeds and
    // the geometry module compiles in stub mode (all operations return errors).
    let occt_found = occt_include.join("Standard.hxx").exists() && has_occt_lib(&occt_lib);
    if occt_found {
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
    // `cam_geometry_bindings` is only emitted when OCCT was found *and*
    // bindgen succeeded — both are required for the symbols to be available
    // at link time.
    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    generate_ffi_bindings(&out_path, occt_found);

    // Declare the custom cfg so Clippy and rustc don't warn on #[cfg(cam_geometry_bindings)].
    println!("cargo:rustc-check-cfg=cfg(cam_geometry_bindings)");

    // Incremental rebuild triggers.
    println!("cargo:rerun-if-changed=cpp/cam_geometry.h");
    println!("cargo:rerun-if-changed=cpp/cam_geometry.cpp");
    println!("cargo:rerun-if-changed=cpp/handle_registry.cpp");
    println!("cargo:rerun-if-changed=cpp/third_party/Clipper2/Clipper2Lib/src/clipper.engine.cpp");
    println!("cargo:rerun-if-changed=cpp/third_party/Clipper2/Clipper2Lib/src/clipper.offset.cpp");
    println!(
        "cargo:rerun-if-changed=cpp/third_party/Clipper2/Clipper2Lib/src/clipper.rectclip.cpp"
    );
    println!(
        "cargo:rerun-if-changed=cpp/third_party/Clipper2/Clipper2Lib/src/clipper.triangulation.cpp"
    );
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
    // Check for the most fundamental OCCT library in all supported forms.
    // .dylib is the macOS shared library extension (Homebrew does not install .so).
    [
        "libTKernel.a",
        "libTKernel.so",
        "libTKernel.dylib",
        "TKernel.lib",
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
            "cpp/third_party/Clipper2/Clipper2Lib/src/clipper.triangulation.cpp",
        ])
        // Suppress warnings from OCCT and Clipper2 headers we do not control.
        .warnings(false);

    // Windows: CRT linkage must match the vcpkg x64-windows-static OCCT build.
    // vcpkg's x64-windows-static triplet compiles with /MT (static CRT).
    // Use static_crt() rather than .flag("/MT") to avoid the D9025 warning
    // that arises when cc's default /MD flag is later overridden by /MT.
    #[cfg(target_os = "windows")]
    build.static_crt(true);

    build.compile("cam_geometry");
}

// ── OCCT link directives ──────────────────────────────────────────────────────

// Libraries present in all supported OCCT versions (7.6.x – 7.9.x).
// Note: TKXCAF is intentionally omitted here; it is appended after the
// STEP/DE libs in link_occt() because TKDESTEP/TKDESTL depend on it.
const OCCT_LIBS_COMMON: &[&str] = &[
    "TKernel",
    "TKMath",
    "TKG2d",
    "TKG3d",
    "TKGeomBase",
    "TKGeomAlgo",
    "TKBRep",
    "TKTopAlgo",
    "TKPrim",
    "TKBO",
    "TKShHealing",
    "TKOffset",
    "TKMesh",
    "TKXSBase",
];

// STEP/IGES/STL library names for OCCT < 7.8 (e.g. Ubuntu 24.04 ships 7.6.3).
const OCCT_STEP_LIBS_PRE78: &[&str] = &["TKSTEPBase", "TKSTEPAttr", "TKSTEP", "TKIGES", "TKSTL"];

// STEP/IGES/STL library names for OCCT 7.8+ (DE framework rename used by
// Homebrew on macOS and vcpkg on Windows).
const OCCT_STEP_LIBS_78PLUS: &[&str] = &["TKDESTEP", "TKDEIGES", "TKDESTL"];

/// Returns true when the OCCT lib dir contains the pre-7.8 `TKSTEPBase` library.
fn has_legacy_step_libs(dir: &std::path::Path) -> bool {
    [
        "libTKSTEPBase.a",
        "libTKSTEPBase.so",
        "libTKSTEPBase.dylib",
        "TKSTEPBase.lib",
    ]
    .iter()
    .any(|name| dir.join(name).exists())
}

// Windows system libraries required by OCCT.
#[cfg(target_os = "windows")]
const WINDOWS_SYSTEM_LIBS: &[&str] = &[
    "Ws2_32", "User32", "Advapi32", "Shell32", "Ole32", "OleAut32", "Gdi32", "Winspool",
];

fn link_occt(occt_lib: &std::path::Path) {
    println!("cargo:rustc-link-search=native={}", occt_lib.display());

    // Windows/vcpkg provides static libs; Linux/macOS apt/brew provide shared libs.
    #[cfg(target_os = "windows")]
    let link_kind = "static";
    #[cfg(not(target_os = "windows"))]
    let link_kind = "dylib";

    for lib in OCCT_LIBS_COMMON {
        println!("cargo:rustc-link-lib={link_kind}={lib}");
    }

    // OCCT 7.8 renamed TKSTEPBase/TKSTEPAttr/TKSTEP/TKIGES/TKSTL.
    // Probe for TKSTEPBase to detect which naming scheme is in use.
    let step_libs = if has_legacy_step_libs(occt_lib) {
        OCCT_STEP_LIBS_PRE78
    } else {
        OCCT_STEP_LIBS_78PLUS
    };
    for lib in step_libs {
        println!("cargo:rustc-link-lib={link_kind}={lib}");
    }
    // The STEP/DE libs depend on TKXCAF, so TKXCAF must come after them.
    println!("cargo:rustc-link-lib={link_kind}=TKXCAF");

    #[cfg(target_os = "windows")]
    for lib in WINDOWS_SYSTEM_LIBS {
        println!("cargo:rustc-link-lib={lib}");
    }
}

// ── FFI binding generation ────────────────────────────────────────────────────

/// Generate `ffi_generated.rs` from `cpp/cam_geometry.h` using bindgen.
///
/// `cam_geometry_bindings` is only emitted when `occt_found` is true *and*
/// bindgen succeeds — both are required for the symbols to be defined at link
/// time.  If either condition fails, a placeholder file is written and the cfg
/// is left unset, causing all `#[cfg(cam_geometry_bindings)]` blocks to compile
/// in stub mode (operations return errors without referencing any C symbols).
///
/// On macOS, set `LIBCLANG_PATH` to the Homebrew LLVM lib directory:
///   export LIBCLANG_PATH=$(brew --prefix llvm)/lib
/// On Windows, set `LIBCLANG_PATH` to the LLVM installation (not the MSVC
/// toolchain — a separate LLVM install is required for bindgen).
fn generate_ffi_bindings(out_path: &std::path::Path, occt_found: bool) {
    if !occt_found {
        // OCCT was not compiled in; write a placeholder and skip the cfg so
        // all #[cfg(cam_geometry_bindings)] blocks compile as stubs.
        std::fs::write(
            out_path.join("ffi_generated.rs"),
            "// FFI bindings skipped — OCCT not found.\n",
        )
        .expect("failed to write placeholder ffi_generated.rs");
        return;
    }

    let result = bindgen::Builder::default()
        .header("cpp/cam_geometry.h")
        .allowlist_function("cg_.*")
        .allowlist_type("Cg.*")
        .allowlist_var("CG_.*")
        .rustified_enum("CgError")
        .rustified_enum("CgSurfaceType")
        .rustified_enum("CgBoolOp")
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
