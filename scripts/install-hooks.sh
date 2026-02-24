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
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

echo "[pre-commit] Running TypeScript type check..."
pnpm typecheck

echo "[pre-commit] All checks passed."
HOOK

chmod +x "$HOOK_FILE"
echo "Pre-commit hook installed at: $HOOK_FILE"
