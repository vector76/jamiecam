#!/usr/bin/env bash
# Installs JamieCam git pre-commit hooks.
#
# The pre-commit hook runs:
#   1. cargo fmt --check  — fails if Rust code is not formatted
#   2. cargo clippy       — fails on any Clippy warning
#   3. pnpm typecheck     — fails on TypeScript type errors
#
# Usage (from repo root):
#   ./scripts/install-hooks.sh

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"
HOOK_FILE="$HOOKS_DIR/pre-commit"

# ── Write the hook script ─────────────────────────────────────────────────────
cat > "$HOOK_FILE" <<'HOOK'
#!/usr/bin/env bash
# JamieCam pre-commit hook
# Auto-installed by scripts/install-hooks.sh

set -euo pipefail

echo "[pre-commit] Checking Rust formatting..."
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

echo "[pre-commit] Running Clippy..."
# Clippy requires GTK/WebKit system headers to compile the full Tauri crate graph.
# In CI (where GTK is installed) this runs fully.  In environments without the
# system libraries, we detect the absence and skip gracefully rather than blocking
# every commit with a build-system error unrelated to Rust code quality.
if pkg-config --exists gtk+-3.0 2>/dev/null; then
  cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
else
  echo "[pre-commit] GTK system libraries not found — skipping Clippy (will run in CI)."
fi

echo "[pre-commit] Running TypeScript type check..."
# Use pnpm if available (preferred), otherwise fall back to npm.
if command -v pnpm &>/dev/null; then
  pnpm typecheck
else
  npm run typecheck
fi

echo "[pre-commit] All checks passed."
HOOK

chmod +x "$HOOK_FILE"
echo "Pre-commit hook installed at: $HOOK_FILE"
