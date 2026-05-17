#!/usr/bin/env bash
# Install the JamieCam pre-commit hook into the local .git/hooks dir.
# Idempotent: re-running overwrites the existing hook with the current version.
set -euo pipefail

# Use --git-path so this works in linked worktrees too (where `.git` is
# a file pointing into the main repo's `.git/worktrees/<name>/` dir).
HOOK_PATH="$(git rev-parse --git-path hooks/pre-commit)"

cat >"$HOOK_PATH" <<'HOOK'
#!/usr/bin/env bash
# JamieCam pre-commit hook (web build).
set -euo pipefail

echo "[pre-commit] Checking Rust formatting..."
cargo fmt --manifest-path src-rust/Cargo.toml --all -- --check

echo "[pre-commit] Running Clippy..."
cargo clippy --manifest-path src-rust/Cargo.toml --lib --all-targets -- -D warnings

echo "[pre-commit] Running TypeScript type check..."
if command -v pnpm &>/dev/null; then
  pnpm typecheck
else
  npx --yes pnpm@10.30.2 typecheck
fi

echo "[pre-commit] All checks passed."
HOOK

chmod +x "$HOOK_PATH"
echo "Installed pre-commit hook at $HOOK_PATH"
