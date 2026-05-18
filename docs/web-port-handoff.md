# JamieCam: Web Port Handoff

> **Audience:** A future agent (or developer) picking up this project without
> access to the conversation that produced the current state. Read this whole
> document before making changes.

## TL;DR

JamieCam was a Tauri desktop CAM app with a Rust+C++ backend (OCCT, Clipper2)
and a React/TypeScript frontend. As of commit `740ba62`
(`refactor!: port from Tauri desktop to static web`), it has been
**pivoted to a static web app** whose compute core is the same Rust code
compiled to WebAssembly via `wasm-pack`.

**Mode 1 (G-code Viewer)** and **Mode 2 (2D Profile cuts, MVP)** both
ship today. All other modes (2.5D, 3D, 2+rotary, 3+rotary, 5-axis), the
OCCT FFI, the original toolpath planner, and all 43 `#[tauri::command]`
handlers were deleted, not just hidden. Mode 2 was re-introduced in
Phase 4 in profile-only form on top of a new pure-Rust stack
(`clipper2-rust`, `usvg`, `dxf`) — see `docs/phase-4-design.md` for the
decisions that shaped it.

The deployment target is GitHub Pages; the deploy workflow ships and
publishes on push to `main`.

## Origin: the Tauri → web pivot

The original app was 7 "modes" sharing a unified desktop UI:

| Mode | Description | Status today |
|---|---|---|
| 1 | G-code Viewer | **Shipped (Phase 1)** |
| 2 | 2D Profile cuts (SVG/DXF → GRBL G-code) | **Shipped (Phase 4, profile-only MVP)** |
| 2.5D | 2D with multi-depth | Deleted |
| 3 | 3D (heightmap/STL/STEP) | Deleted |
| 2+rotary | 2D wrapped on rotary axis | Deleted |
| 3+rotary | 3D + rotary | Deleted |
| 5-axis | STEP/IGES + 5-axis | Deleted |

Mode 2 today covers **profile cuts only** — pocket clearing, drilling,
island pockets, and tab retention are deliberately out of scope for the
first ship; they will land as follow-ups. See
`docs/phase-4-design.md` §5.

The user's decisions that shaped the pivot:
- **No backend.** Targets GitHub Pages and similar static hosts. No
  ability to run a server-side OCCT or anything else.
- **Tauri abandoned entirely.** No dual-build, no compatibility shims,
  no preserved IPC abstractions.
- **Delete dead modes rather than stubbing.** When the future modes come
  back, they'll be re-introduced one by one.
- **Hybrid Rust→WASM + TypeScript glue** is the architecture, not a
  pure-TS rewrite.
- **Mode 1 first** because it has zero dependencies on Clipper2/OCCT and
  proves the whole pipeline end-to-end.
- **Project persistence is download/upload of `.jcam` zips + an
  IndexedDB Recents cache.** Shipped in Phase 2; the `workingEnv`
  IndexedDB store added in Phase 4 follows the same "browser-local,
  no server" framing.

## Architecture

### Layout

```
jamiecam/
├── src-rust/              # Rust crate, cdylib + rlib (was src-tauri/)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs         # module declarations only
│       ├── wasm.rs        # #[wasm_bindgen] surface, _inner functions
│       ├── types.rs       # Vec3, BoxDimensions, StockDefinition,
│       │                  # MeshData, FaceGroup, LineGeometryData
│       ├── error.rs       # AppError (serde-tagged enum)
│       ├── parse_warning.rs  # Shared ParseWarning shape (line + message)
│       ├── gcode_parser/  # Pure-Rust G-code parser (Mode 1)
│       ├── dexel/         # Pure-Rust dexel material-removal engine
│       │                  # (used by both modes via the worker)
│       ├── parsers/       # SVG (usvg) and DXF (dxf) → Polyline (Mode 2)
│       ├── clipper/       # Thin facade over clipper2-rust for offsets
│       ├── geometry2d/    # Point2 / Polyline / Region — the shared
│       │                  # 2D millimetre types Mode 2 passes around
│       ├── profile/       # Mode 2 profile-cut planner (offset + passes)
│       ├── grbl/          # Hardcoded GRBL G-code emitter
│       └── working_env/   # Mode 2 machine setups, tools, availability
├── src/                   # Frontend (Vite, React 19, TypeScript)
│   ├── main.tsx           # Trivial — just renders <App/>
│   ├── App.tsx            # Mode-aware shell: New Project picker,
│   │                      # Open/Save .jcam, Recents list, mode dispatch
│   ├── api/
│   │   ├── gcodeViewer.ts # Mode 1 wasm bridge + prewarm
│   │   ├── mode2.ts       # Mode 2 wasm bridge (parse, plan, emit)
│   │   ├── simulation.worker.ts        # Web Worker entry: dexel sim
│   │   ├── simulationWorkerClient.ts   # Promise wrapper around the worker
│   │   └── types.ts       # TS mirror of the Rust wasm boundary
│   ├── components/
│   │   ├── modes/ToolpathViewerMode.tsx  # Mode 1 shell
│   │   ├── modes/Mode2ProfileMode.tsx    # Mode 2 shell
│   │   ├── working-env/   # WorkingEnvironmentModal: shared CRUD UI
│   │   │                  #  for setups, tools, availability matrix
│   │   └── ui/            # shadcn primitives (button, scroll-area,
│   │                      #  sidebar-section, etc.)
│   ├── viewport/          # Three.js 3D viewport (Mode 1 + Mode 2 sim preview)
│   ├── viewport2d/        # Canvas2D viewport (Mode 2 primary workspace)
│   ├── persistence/       # .jcam pack/unpack + IndexedDB (recents, workingEnv)
│   ├── store/             # Zustand viewportStore (has dead state — see
│   │                      #  Tech Debt below)
│   ├── lib/utils.ts       # shadcn cn() helper
│   ├── test-setup.ts      # Polyfills (Blob.text, fake-indexeddb) for jsdom
│   ├── wasm-pkg/          # GENERATED by wasm-pack — gitignored
│   └── index.css
├── public/samples/        # Bundled samples: demo-pocket.nc (Mode 1),
│                          # sample-profile.svg, sample-profile.dxf (Mode 2)
├── docs/                   # See "docs/ after the pivot prune" below
├── index.html              # Vite entrypoint
├── vite.config.ts          # vite-plugin-wasm; target es2022
├── tsconfig.json
├── package.json            # packageManager: pnpm@10.30.2
├── pnpm-workspace.yaml     # Single-package workspace
├── eslint.config.js
├── prettier.config.js
├── components.json         # shadcn config
└── CLAUDE.md / AGENTS.md   # Generic TDD/git/docs guidance
```

### The wasm boundary

**Rust side** (`src-rust/src/wasm.rs`):

```rust
// Pure Rust, takes &str so it's testable without wasm
pub fn load_gcode_for_viewer_inner(content: &str)
    -> Result<GcodeViewerLoadResult, AppError>

// Thin wasm-bindgen wrapper that calls _inner
#[wasm_bindgen(js_name = loadGcodeForViewer)]
pub fn load_gcode_for_viewer(content: &str) -> Result<JsValue, JsValue>

// Auto-runs on module load; installs console_error_panic_hook
#[wasm_bindgen(start)]
pub fn wasm_init()
```

The `_inner` + `#[wasm_bindgen]` pair is the same testability pattern the
original Tauri code used. Keep it for future wasm exports.

**TypeScript side** (`src/api/gcodeViewer.ts`):

```ts
async function getWasm(): Promise<WasmModule> {
  if (!wasmPromise) {
    wasmPromise = (async () => {
      try {
        const mod = await import('../wasm-pkg/jamiecam')
        await mod.default()   // initializes wasm + triggers #[start]
        return mod
      } catch (err) {
        wasmPromise = null    // allow retry after failure
        throw err
      }
    })()
  }
  return wasmPromise
}

export async function loadGcodeForViewer(content: string)
    : Promise<GcodeViewerLoadResult>
```

**Serialization contract**:
- Rust structs use `#[derive(Serialize)]` + `#[serde(rename_all = "camelCase")]`.
- `serde-wasm-bindgen` converts to plain JS objects (no Maps).
- `Vec<T>` becomes a plain JS `Array`, **not** a typed array — verified
  by a Node smoke test (see Verification below).
- `Vec<u8>` is a plain array of numbers, **not** a `Uint8Array`.
- TS types in `src/api/types.ts` mirror the Rust serde output exactly.

### State

The wasm module is stateless — every call is a pure function of its
inputs. Frontend state lives in three places:

- **In-memory** per active mode: the loaded G-code (Mode 1) or the
  imported artwork + selection + operation params + generated toolpath
  (Mode 2). Each mode reports its "savable" shape up to the shell as a
  `ProjectState`.
- **`.jcam` zip files** for explicit save/load — see `projectFile.ts`.
  `project.json` carries a required `mode` field (`"gcode-viewer"` or
  `"2d-profile"`); the discriminated payload contains the mode-specific
  state. Mode 2 also persists the original imported SVG/DXF bytes as a
  separate zip entry so projects survive parser changes.
- **IndexedDB (`jamiecam` database)** holds two object stores:
  - `recents` — last-opened projects (keyed by file name), auto-upserted
    so a tab close doesn't lose work.
  - `workingEnv` — the **working environment**: machine setups, tools,
    and the tool↔setup availability matrix, plus the cross-session
    `activeSetupId`. This is intentionally **outside `.jcam`** because
    it describes the user's CNC hardware, not the project. See
    `docs/phase-4-design.md` §6 for the design and
    `src/persistence/workingEnv.ts` for the schema. On first run a
    placeholder setup + tool + availability pair are seeded so Mode 2
    always has something to render.

There is still no IPC, no server, and no shared cross-tab state.

## Build & development

```bash
# One-time setup:
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
npx pnpm@10.30.2 install         # Note: corepack's bundled pnpm is broken
                                  # on Node 20 — see Tooling Quirks below

# Daily workflow:
npx pnpm@10.30.2 dev              # Rebuilds wasm, then starts vite dev server
npx pnpm@10.30.2 test             # Vitest (no wasm needed — tests mock the module)
npx pnpm@10.30.2 typecheck        # Rebuilds wasm, then tsc --noEmit
npx pnpm@10.30.2 lint
npx pnpm@10.30.2 build            # Rebuilds wasm, then production vite build

cd src-rust
cargo test --lib                  # 361 tests
cargo clippy --lib --all-targets -- -D warnings
cargo fmt --check
```

`pnpm wasm:build` runs `wasm-pack build src-rust --target web --out-dir
../src/wasm-pkg --out-name jamiecam`. It is automatically chained into
`dev`, `build`, and `typecheck` because the generated `.d.ts` is what
makes `import('../wasm-pkg/jamiecam')` typecheck. A fresh clone with
just `pnpm install` will have no wasm; the first `pnpm typecheck` or
`pnpm dev` builds it. There is **no** `postinstall` hook (chose not to
gate `pnpm install` on having `wasm-pack` on PATH).

### Pre-commit hook

Install with `scripts/install-hooks.sh` (idempotent — re-running
overwrites). It writes a `.git/hooks/pre-commit` that runs:
1. `cargo fmt --manifest-path src-rust/Cargo.toml --all -- --check`
2. `cargo clippy --manifest-path src-rust/Cargo.toml --lib --all-targets -- -D warnings`
3. `pnpm typecheck`

The hook itself is not tracked; the installer is. Re-run after a fresh
clone so the checks run on every commit.

### Tooling quirks

- **pnpm via corepack is broken on Node 20.20**. The system `pnpm` symlinks
  to corepack's bundled pnpm 11.x which requires Node 22 (it imports
  `node:sqlite`). Workaround: use `npx --yes pnpm@10.30.2 <cmd>`. The
  `packageManager: "pnpm@10.30.2"` field in `package.json` declares the
  right version, but corepack on Node 20 won't honor it.
- **Cargo.lock is gitignored**. Debatable for a deployable wasm artifact;
  matches the project's prior convention. If reproducibility becomes an
  issue, commit it.
- **jsdom 25 doesn't ship `Blob.prototype.text`**. Polyfilled in
  `src/test-setup.ts` via `FileReader`. Don't use `new Response(blob).text()`
  — that uses Node's undici, which doesn't read jsdom's Blob bytes correctly.

## What ships today (Phases 1, 2, 4)

The shell header picks a mode for a New Project, opens a `.jcam`
(autoselects its mode), saves the current project, and lists Recents.

**Mode 1 — G-code Viewer:**
1. "Open G-code…" → pick a `.nc` / `.gcode` / `.tap`, or "Load Sample"
   for `/samples/demo-pocket.nc`.
2. Sidebar shows parsed `@STOCK` / `@TOOL` header metadata if present.
3. Three.js viewport renders toolpath line geometry (rapids grey, cuts
   coloured per tool number).
4. "Simulate" runs the dexel material-removal sim in a Web Worker and
   renders the resulting workpiece mesh.

**Mode 2 — 2D Profile cuts (MVP):**
1. "Working Environment…" opens the modal to edit machine setups, tools,
   and the availability matrix (all persisted to IndexedDB).
2. "Open SVG/DXF…" or "Load Sample…" imports artwork; parsed polylines
   appear in the Paths panel with per-row selection checkboxes.
3. Operation editor: tool dropdown (filtered by the active setup's
   availability), cut-side (outside/inside/onLine), depth/feed/spindle.
4. "Generate" produces a profile toolpath overlay in the Canvas2D view.
5. "Simulate" emits the GRBL G-code, feeds it through the same dexel
   worker Mode 1 uses, and swaps the left pane to the 3D preview.
   "Back to 2D" returns to the Canvas2D view.
6. "Export G-code" downloads the GRBL program. "Save Project" writes a
   `.jcam` that includes the original imported SVG/DXF bytes alongside
   the parsed cache, path selection, and operation params.

What's deliberately **not** there yet:
- Anything from any other mode.
- Mode 2 operations other than profile cuts (no pocket / drill / island
  pocket / tab retention; see `docs/phase-4-design.md` §5).
- Pluggable post-processors (GRBL only; see §7).
- Loading bar for long simulations (just shows "Simulating…").
- Bundle-size optimization (main chunk is still Three.js-heavy).

## Phase roadmap

### Phase 1 — Foundation ✅ shipped (commit 740ba62)

### Phase 2 — Dexel sim + persistence + deploy ✅ shipped

1. Dexel sim exposed via wasm — `d8c2495`.
2. Sim runs in a Web Worker — `4534fcb`.
3. `.jcam` save/load + IndexedDB recents — `b38e92c`.
4. GitHub Pages deploy workflow + `BASE_PATH` — `029590d`.

### Phase 3 — Hardening (mostly done; remaining items deferred)

Done:
- "Initializing engine…" indicator while the wasm module first loads
  (covers the ~189 KB / ~91 KB gz wasm download on first visit).
- `AppError` plumbing verified end-to-end (Rust → wasm → worker → UI),
  with tests at each layer. Sim validation errors surface cleanly.
- Stale `docs/` pruned; surviving forward-looking docs banner-tagged.
- Root `README.md`, `scripts/install-hooks.sh` added.

Explicitly deferred (per `docs/phase-4-design.md` §1 — no user-visible
problem motivates them yet):
- Bundle analysis / Three.js lazy-load.
- SharedArrayBuffer / Rayon threading (would need a host that can serve
  COOP/COEP headers — GH Pages can't).

### Phase 4 — Mode 2 (2D profile cuts) ✅ shipped

Profile-only MVP of Mode 2. The full design and the rationale for each
choice live in `docs/phase-4-design.md`; the short version:

- `clipper2-rust` (pure-Rust port) replaces the deleted C++ Clipper2
  FFI; geometry stays Rust-side.
- `usvg` + `dxf` crates parse SVG and DXF into the new
  `geometry2d::{Point2, Polyline, Region}` types.
- Profile toolpath generator (`profile::generate_profile`) applies a
  tool-radius offset and step-down passes.
- Hardcoded GRBL emitter (`grbl`) — no pluggable post-processor yet.
- Working-environment model (`working_env`): machine setups, tools, and
  the tool↔setup availability matrix, persisted to the IndexedDB
  `workingEnv` store.
- `.jcam` gains a required `mode` field; old files default to
  `"gcode-viewer"`. Mode 2 `.jcam` files persist the unmodified
  imported bytes alongside the parsed cache.
- The planner stays 2D-only — no speculative `GeometrySource` enum
  (`phase-4-design.md` §4). The next non-2D consumer gets to design the
  abstraction against a real second user.
- Mode 2 simulation routes through the GRBL emitter into the existing
  dexel worker (`phase-4-design.md` §5, route (a)), so what's previewed
  is exactly what's exported.
- Canvas2D component is the primary Mode 2 workspace; the Three.js
  viewport handles only the sim preview.

Follow-up Mode 2 work (out of scope for the first ship): pocket
clearing, drilling, island pockets, tab retention; multi-dialect
post-processor; editor UX polish for setups/tools beyond the minimal
modal.

### Phases 5+ — Other modes

Each mode is roughly: get the mode's Rust modules wasm-compatible,
expose via wasm-bindgen, add a TS API wrapper, wire UI.

**Mode 3 (3D, heightmap/STL only)** is a plausible next target. STL is
pure Rust. STEP/IGES requires OCCT.

**Modes 4–6 (rotary)** are mostly the same as 3 with extra kinematics.

**Mode 7 (5-axis)** depends on OCCT. See "OCCT possibilities" below.

### OCCT possibilities (for STEP/IGES)

OCCT *can* compile to WASM — `opencascade.js` is a maintained binding.
The trade-offs (verified during the Phase 1 planning):

- A full OCCT WASM build is large (~10–40 MB depending on selected
  modules). Custom builds can include only what you need.
- Can be **lazy-loaded**: dynamic `import()` of an `occt.js` chunk
  triggered only when the user clicks "Import STEP". `WebAssembly` and
  HTTP caching keep subsequent loads fast.
- Two separate WASM modules (the core jamiecam wasm + the OCCT wasm)
  can't share memory; you'd pass geometry between them as serialized
  bytes (typically: OCCT does STEP → triangle mesh, then everything else
  operates on the mesh).
- Code-splitting the Rust side: split OCCT-touching Rust into a second
  crate so the OCCT wasm artifact is genuinely separate from the
  always-loaded core.

This is a Phase-5+ discussion. Don't take it on early.

## Tech debt to be aware of

1. **`src/store/viewportStore.ts` carries dead state.** It still has
   fields and methods for face selection (`selectionMode`,
   `hoveredFaceIdx`, `selectedFaceFingerprints`, `faceDescriptors`,
   `toggleFaceSelection`, `setFaceDescriptors`, etc.), measurement
   history, and simulation playback. The face-selection state is dead
   until STEP import returns (Phase 5+); the measurement state is wired
   to working toolbar buttons in `Viewport.tsx`. Trimming requires also
   editing `Viewport.tsx` and `viewportStore.test.ts`.

2. **`Viewport.tsx` toolbar has 5 buttons (Persp/Ortho, display mode,
   distance, angle, clear measurements).** All function, but the
   "display mode" dropdown affects the model mesh which neither mode
   loads, so 3 of its 4 options are no-ops today.

3. **`api/types.ts::FaceDescriptor`** is a stub kept in shape with the
   original (5 required fields) just to satisfy `viewportStore`. No
   shipped mode produces one.

4. **`docs/` was pruned post-pivot** — see below.

## `docs/` after the pivot prune

Pre-pivot docs describing deleted Tauri/OCCT/CAM-algorithm code were
deleted. What remains:

- `web-port-handoff.md` — this file; the live source of current-state truth.
- `phase-4-design.md` — the locked-in Mode 2 design decisions.
- `gcode-parser.md`, `dexel-material-removal.md`, `tool-geometry-model.md`
  — specs for code that still ships. Carry a post-pivot banner noting the
  old Tauri path references are stale.
- `roadmap.md`, `modes-overview.md` — forward-looking multi-mode plan.
  Carry a banner noting the "Done" shared-infrastructure rows that
  describe deleted Tauri code (OCCT, original post-processor, etc.)
  must be reintroduced in WASM-compatible form before the modes that
  depend on them can land.

## Verification checklist

Confirms the project is in a working state. Run after pulling, before
starting work, or after a non-trivial change:

```bash
npx --yes pnpm@10.30.2 install
npx --yes pnpm@10.30.2 lint              # 0 warnings
npx --yes pnpm@10.30.2 test               # 438 / 438
npx --yes pnpm@10.30.2 typecheck          # no errors
npx --yes pnpm@10.30.2 build              # produces dist/

cd src-rust
cargo test --lib                          # 361 / 361
cargo clippy --lib --all-targets -- -D warnings
cargo fmt --check
```

End-to-end browser smoke test:
```bash
npx --yes pnpm@10.30.2 vite preview --port 4173 --host 127.0.0.1 &
sleep 5
curl -s http://127.0.0.1:4173/ | head -5                                   # SPA HTML
curl -sI http://127.0.0.1:4173/samples/demo-pocket.nc | head -3            # 200
curl -sI http://127.0.0.1:4173/samples/sample-profile.svg | head -3        # 200
curl -sI http://127.0.0.1:4173/samples/sample-profile.dxf | head -3        # 200
curl -sI http://127.0.0.1:4173/assets/jamiecam_bg-*.wasm | head -3         # 200, application/wasm
```

In a browser:
1. Mode 1: stay on the default "G-code Viewer" mode, click "Load Sample",
   confirm the viewport shows toolpath lines and the sidebar shows the
   stock and tool metadata from the sample's `@STOCK`/`@TOOL` headers.
2. Mode 2: switch the shell to "2-D Profile", "Load Sample…" →
   `sample-profile.svg`, pick the seeded tool, click Generate then
   Simulate, and confirm the 3-D preview shows the carved sample.

## Conversational ground rules (from the user)

These came up explicitly in the pivot conversation. Carry them forward:

- **Don't mention Claude Code as coauthor in commits.**
- **Never use `git -C ...`.** Always `pwd` first to confirm directory.
- **TDD is the convention.** Write tests alongside (or before) code.
  See `CLAUDE.md` / `AGENTS.md`.
- **The user prefers code as source of truth** over detailed inventory
  docs that go stale (this whole document being the explicit exception
  for handoff).
- **Pivot decisions are settled.** Don't relitigate "should we keep
  Tauri" / "should we preserve Mode 7" / etc. The user committed to the
  web-only + Mode-1-first approach.
- **Phase 4 (Mode 2) decisions are also settled.** See
  `docs/phase-4-design.md` — read it before proposing alternatives to
  `clipper2-rust`, the profile-only MVP scope, the GRBL-only emitter,
  the working-environment storage shape, the Canvas2D split, or the
  decision to keep the planner 2D-only.
