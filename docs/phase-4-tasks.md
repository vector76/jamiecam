# Phase 4 Tasks

Short handoff for whoever takes on Phase 4 (Mode 2: 2D profile cuts).
The authoritative design and rationale live in `phase-4-design.md` —
this document is the entry point and the task list.

## In scope: seven tasks

| # | Task                                                    | Design ref |
|---|---------------------------------------------------------|------------|
| 1 | `.jcam` `mode` field                                    | §8         |
| 2 | `clipper2-rust` integration                             | §2         |
| 3 | SVG + DXF parsers (`usvg`, `dxf` crates)                | §3         |
| 4 | Profile toolpath generator                              | §4 + §5    |
| 5 | Working-environment data model (machine setups + tools) | §6         |
| 6 | GRBL G-code emitter                                     | §7         |
| 7 | Canvas2D viewport component                             | §9         |

For each task, read the referenced section of `phase-4-design.md`
before starting. That section gives the choice of library or data
shape, the constraints the decision was made under, and the
alternatives that were rejected.

## Order

Tasks 1, 2, 3, 5, and 7 are independent and can be tackled in any
order or in parallel. Task 4 depends on 2 and 3 (and either 5 or
inlined tool params for the first slice); task 6 depends on 4. Full
dependency graph in §10 of `phase-4-design.md`.

## Explicitly out of scope

- Mode 2 operations other than profile cuts — no pocket, drill, island
  pocket, or tab retention in this phase. See §5.
- Multi-dialect post-processor — GRBL only. See §7.
- Phase 3 hardening (bundle/lazy-load, threading). See §1.

## Open questions to resolve during the work

These are real design gaps the doc does not answer; resolve them at
the natural moment rather than up front.

- ~~**Simulation pathway.**~~ Resolved 2026-05-17: route through
  task 6 (GRBL G-code) into the existing dexel worker. See
  `phase-4-design.md` §5 "Simulation pathway" — task 6 is on the
  critical path for the Mode 2 Simulate button.
- **Mode 2 UI shell** (operation editor, file picker, panel layout)
  is not a design decision yet. Scope it when reaching that work.
- **Project ↔ machine-setup reference** field in `project.json` is
  implied by §6 but not specified. Pin the shape when working on
  task 5 and/or task 1.

---

*Related documents: `phase-4-design.md` (decisions and rationale),
`roadmap.md` (multi-mode plan), `web-port-handoff.md` (current state
of the codebase).*
