<!--
Sync Impact Report:
- Version change: 0.0.0 → 1.0.0
- Modified principles: N/A (initial ratification)
- Added sections: Core Principles (6), Technical Constraints, Development Workflow, Governance
- Removed sections: N/A
- Templates requiring updates:
  - .specify/templates/plan-template.md ✅ compatible (Constitution Check section exists)
  - .specify/templates/spec-template.md ✅ compatible (no constitution-specific content)
  - .specify/templates/tasks-template.md ✅ compatible (no constitution-specific content)
- Follow-up TODOs: None
-->

# WZed Constitution

## Core Principles

### I. Minimalist Scope

WZed is a text editor and nothing else. No debugger, no terminal, no
extensions, no collaboration, no plugin system. Every feature request MUST be
evaluated against this scope — if it is not text editing, it does not belong.
Rationale: scope creep is the fastest path to an unmaintainable side project.

### II. Error Resilience

Code MUST NOT panic in production. `unwrap()` is forbidden in non-test code.
Errors MUST be propagated with `?`, logged with `.log_err()`, or handled
explicitly with `match`/`if let`. Silent error discards (`let _ =`) are
forbidden. Rationale: an editor that crashes loses user data.

### III. No External Runtime Dependencies

The single binary MUST be self-contained at runtime. Language grammars, themes,
and assets are compiled in. The only runtime dependency is a local Zed source
checkout at build time (`../zed`). Rationale: the editor should work after
`cargo build` with no further setup.

### IV. Existing File Discipline

New functionality MUST be added to existing source files unless it is a clearly
new logical component. Creating many small files is forbidden. `mod.rs` is
forbidden. Rationale: a ~4000-line codebase should not have 30 source files.

### V. Zed Framework Delegation

Heavy lifting (text buffer, syntax highlighting, UI rendering, language
infrastructure) MUST be delegated to Zed's crates. WZed is the thin
application shell — it wires components together, it does not reimplement them.
Rationale: duplicating Zed functionality is wasted effort and doubles the
maintenance burden.

### VI. Session and Data Safety

User data (open tabs, unsaved content, session state) MUST survive crashes and
unexpected exits. Autosave with snapshot backups, atomic file writes
(write-to-tmp-then-rename), and session persistence are mandatory for any
state-changing operation. Rationale: losing user work is the worst failure mode
for an editor.

## Technical Constraints

- **Language**: Rust edition 2024
- **UI Framework**: Zed GPUI (via path dependencies on `../zed/crates/*`)
- **Architecture**: Single binary, single window, single `LiteWorkspace` entity
- **Platforms**: Linux (primary), Windows (secondary)
- **Configuration**: `~/.config/wzed/` (settings.json, keymap.json,
  session.json, recent.json)
- **Build**: `cargo build` — no build scripts, no code generation, no extra
  tooling required
- **Testing**: `cargo test` for unit tests (pure logic modules); manual
  integration testing via `test-step.md` for UI and IPC

## Development Workflow

1. All changes MUST compile with `cargo build` and pass `cargo test`
2. `clippy.toml` disallows `unwrap()` — violations MUST be fixed before commit
3. Comments explain "why", never "what" — the code itself is the documentation
   of what it does
4. Variable names use complete words — no abbreviations
5. Entity closures use the inner `cx`, never the outer one
6. Commit messages are written in English

## Governance

This constitution is the authoritative source for project principles and
constraints. All code changes, feature requests, and architectural decisions
MUST comply with the principles above.

Amendments require:
1. A concrete justification for why the change is needed
2. Verification that no existing code violates the amended principle
3. Updates to all dependent templates and documentation

Compliance is verified by code review and `cargo clippy`. Any PR that violates
a principle MUST be rejected unless the principle is amended first.

**Version**: 1.0.0 | **Ratified**: 2026-06-13 | **Last Amended**: 2026-06-13
