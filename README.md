# JamieCam

A browser-based G-code viewer and material-removal simulator. Load a
`.nc` / `.gcode` / `.tap` file, see the toolpath in 3D, and run a dexel
simulation to preview the cut workpiece.

The compute core is Rust compiled to WebAssembly; the UI is React +
Three.js. No backend — deploys as a static site.

## Quick start

Prereqs: Node 20+ and Rust (stable). The build uses `pnpm@10.30.2` and
`wasm-pack`.

```bash
rustup target add wasm32-unknown-unknown
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh   # or: cargo install wasm-pack
npx --yes pnpm@10.30.2 install
npx --yes pnpm@10.30.2 dev
```

`npx --yes pnpm@10.30.2` is used in place of bare `pnpm` because
corepack's bundled pnpm on Node 20 imports `node:sqlite` and won't run.
On Node 22 you can drop the `npx` prefix.

The `dev`, `build`, and `typecheck` scripts all rebuild the wasm module
first; a fresh clone needs no separate wasm step.

## Testing

```bash
npx --yes pnpm@10.30.2 test          # frontend (vitest)
cargo test --manifest-path src-rust/Cargo.toml --lib
```

## Pre-commit hook

```bash
./scripts/install-hooks.sh
```

Installs a hook that runs `cargo fmt --check`, `cargo clippy`, and
`pnpm typecheck` before each commit.

## Deployment

`main` deploys to GitHub Pages automatically via
`.github/workflows/deploy.yml`.

## Status & roadmap

Only Mode 1 (G-code Viewer + simulation) ships today. The pivot from a
Tauri desktop app to a static web app deleted the multi-mode CAM engine;
modes are being reintroduced one at a time in WASM-compatible form.

- `docs/web-port-handoff.md` — current state, architecture, tech debt.
- `docs/roadmap.md` — forward plan for the remaining modes.
