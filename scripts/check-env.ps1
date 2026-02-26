# JamieCam environment check script - Windows (PowerShell)
#
# Verifies that all required tools and environment variables are present
# before attempting a build.  Exits with a non-zero code if any check fails.
#
# Usage (from repo root, in PowerShell):
#   .\scripts\check-env.ps1
#
# See docs/windows-build-setup.md for installation instructions.

#Requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'

# -- Minimum required versions -------------------------------------------------
$MIN_RUSTC_VER   = [Version]'1.77.0'
$MIN_NODE_MAJOR  = 20
$MIN_PNPM_MAJOR  = 9
$MIN_CMAKE_VER   = [Version]'3.20.0'

# -- Result counters -----------------------------------------------------------
$script:PassCount = 0
$script:FailCount = 0

function Write-Pass([string]$Message) {
    Write-Host "  [PASS] $Message" -ForegroundColor Green
    $script:PassCount++
}

function Write-Fail([string]$Message) {
    Write-Host "  [FAIL] $Message" -ForegroundColor Red
    $script:FailCount++
}

# -- Version helpers -----------------------------------------------------------

function Get-VersionFromOutput([string]$Output) {
    # Extracts the first x.y.z version string from a command's output
    if ($Output -match '(\d+\.\d+\.\d+)') { return [Version]$Matches[1] }
    return $null
}

# -- Individual checks ---------------------------------------------------------

function Test-Rustc {
    $cmd = Get-Command rustc -ErrorAction SilentlyContinue
    if (-not $cmd) {
        Write-Fail "rustc not found - install Rust: https://rustup.rs"
        return
    }
    $output = & rustc --version 2>&1
    $ver = Get-VersionFromOutput $output
    if ($ver -ge $MIN_RUSTC_VER) {
        Write-Pass "rustc $ver (>= $MIN_RUSTC_VER)"
    } else {
        Write-Fail "rustc $ver is below minimum $MIN_RUSTC_VER - run: rustup update stable"
    }
}

function Test-Cargo {
    $cmd = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cmd) {
        Write-Fail "cargo not found - install Rust: https://rustup.rs"
        return
    }
    $output = & cargo --version 2>&1
    $ver = Get-VersionFromOutput $output
    Write-Pass "cargo $ver"
}

function Test-Node {
    $cmd = Get-Command node -ErrorAction SilentlyContinue
    if (-not $cmd) {
        Write-Fail "node not found - install Node.js $MIN_NODE_MAJOR LTS: https://nodejs.org"
        return
    }
    $output = & node --version 2>&1
    $ver = Get-VersionFromOutput $output
    if ($ver -and $ver.Major -ge $MIN_NODE_MAJOR) {
        Write-Pass "node $ver (>= $MIN_NODE_MAJOR)"
    } else {
        Write-Fail "node $ver is below minimum v$MIN_NODE_MAJOR - update Node.js"
    }
}

function Test-Pnpm {
    $cmd = Get-Command pnpm -ErrorAction SilentlyContinue
    if (-not $cmd) {
        Write-Fail "pnpm not found - run: npm install -g pnpm"
        return
    }
    $output = & pnpm --version 2>&1
    $ver = Get-VersionFromOutput $output
    if ($ver -and $ver.Major -ge $MIN_PNPM_MAJOR) {
        Write-Pass "pnpm $ver (>= $MIN_PNPM_MAJOR)"
    } else {
        Write-Fail "pnpm $ver is below minimum $MIN_PNPM_MAJOR - run: npm install -g pnpm"
    }
}

function Test-Cmake {
    $cmd = Get-Command cmake -ErrorAction SilentlyContinue
    if (-not $cmd) {
        Write-Fail "cmake not found - install cmake >= $MIN_CMAKE_VER"
        return
    }
    $output = & cmake --version 2>&1
    $ver = Get-VersionFromOutput ($output | Select-Object -First 1)
    if ($ver -ge $MIN_CMAKE_VER) {
        Write-Pass "cmake $ver (>= $MIN_CMAKE_VER)"
    } else {
        Write-Fail "cmake $ver is below minimum $MIN_CMAKE_VER - upgrade cmake"
    }
}

function Test-OcctInclude {
    $dir = "$env:OCCT_INCLUDE_DIR"
    if (-not $dir) {
        Write-Fail "OCCT_INCLUDE_DIR is not set - see docs/windows-build-setup.md"
        return
    }
    $header = Join-Path $dir 'Standard.hxx'
    if (Test-Path $header) {
        Write-Pass "OCCT_INCLUDE_DIR=$dir (Standard.hxx found)"
    } else {
        Write-Fail "OCCT_INCLUDE_DIR=$dir set but Standard.hxx not found"
    }
}

function Test-OcctLib {
    $dir = "$env:OCCT_LIB_DIR"
    if (-not $dir) {
        Write-Fail "OCCT_LIB_DIR is not set - see docs/windows-build-setup.md"
        return
    }
    $lib = Get-ChildItem -Path $dir -Filter 'TKBRep.lib' -ErrorAction SilentlyContinue
    if ($lib) {
        Write-Pass "OCCT_LIB_DIR=$dir (TKBRep.lib found)"
    } else {
        Write-Fail "OCCT_LIB_DIR=$dir set but TKBRep.lib not found"
    }
}

function Test-Libclang {
    $dir = "$env:LIBCLANG_PATH"
    if (-not $dir) {
        Write-Fail "LIBCLANG_PATH is not set - see docs/windows-build-setup.md"
        return
    }
    $lib = Get-ChildItem -Path $dir -Filter 'libclang*' -ErrorAction SilentlyContinue
    if ($lib) {
        Write-Pass "LIBCLANG_PATH=$dir (libclang found)"
    } else {
        Write-Fail "LIBCLANG_PATH=$dir set but libclang not found"
    }
}

# -- Main ----------------------------------------------------------------------
Write-Host "JamieCam environment check"
Write-Host "=========================="
Write-Host ""
Write-Host "Toolchain:"
Test-Rustc
Test-Cargo
Test-Node
Test-Pnpm
Test-Cmake
Write-Host ""
Write-Host "Environment variables:"
Test-OcctInclude
Test-OcctLib
Test-Libclang
Write-Host ""
Write-Host "Results: $($script:PassCount) passed, $($script:FailCount) failed"

if ($script:FailCount -gt 0) {
    Write-Host "Environment check FAILED - fix the issues above before building." -ForegroundColor Red
    Write-Host "See docs/windows-build-setup.md for setup instructions."
    exit 1
} else {
    Write-Host "Environment check PASSED - ready to build!" -ForegroundColor Green
}
