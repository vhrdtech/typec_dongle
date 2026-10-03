# Working on typec_dongle (Donguru)

Guidance for AI agents and contributors. Read this before changing code.

Donguru (ドングル) is a USB Type-C dongle (board B153, STM32G0B1CE) with a USB hub, USART on SBU with automatic TX/RX
flip, I2C / GPIO / ADC, a power switch and a power meter, plus a scriptable CLI (`donguru` / `dg`) that drives it
over USB with WireWeaver. Hardware is at rev A bring-up; rev B goes to first testers end of 2026, and v1.0 (dongle,
Rust driver, Python, CLI, minimal GUI) is planned for a CrowdSupply launch. Board files live in the `hw` repo,
`hw/b153_typec_serial`. Two people work here: keep to the conventions below so both can follow each other's work.

The firmware has its own [fw/donguru/AGENTS.md](fw/donguru/AGENTS.md) from the embedded_bedrock firmware template:
read it for the template layout, `bedrock_fw.json` and the template upgrade procedure. The design of record for the
CLI and packaging is in [docs/design/](docs/design/index.md).

## FEATURES.md is the source of truth

[FEATURES.md](FEATURES.md) lists every feature with its status, every known bug, and what is planned, with stable
IDs per area (`CLI-3`, `FW-5`, ...). The design docs say what a feature should look like; FEATURES.md says where it
stands.

- **Read the relevant area before starting.** A CLI command often needs a firmware API item first; the items say so.
- **Name IDs with a short slug when talking to the user** (answers, plans, summaries, tables):
  `CLI-5 gpio-commands`, never a bare `CLI-5`. The slug is 2-4 kebab-case words from the item's title. Commit
  messages, CHANGELOG and code `TODO`s keep the bare ID.
- **Update it in the same commit** as the code: mark items ✅/🚧/🧪/🐛 with a pointer to the code, add bugs you
  find but don't fix (next free ID of the area), move obsolete items to *Dropped and superseded*. Never renumber
  or reuse IDs. ✅ for anything touching hardware means tested on a board: say which revision. Built but not tested
  on hardware is 🧪.
- Don't track status anywhere else (README checklists, TODO files). Code `TODO`s that matter reference an ID:
  `// TODO(FW-8): ...`.

## CHANGELOG.md records every change

[CHANGELOG.md](CHANGELOG.md) is the history, FEATURES.md the current state; keep both.

- Every change a user of the dongle or the CLI would notice gets an entry under `## [Unreleased]` in the same
  commit: `### Added`, `### Changed`, `### Fixed`, `### Removed`. Short, with the feature ID in parentheses.
- Always record: firmware API changes (with the new `donguru_api` version and whether old CLIs still work), CLI
  output or exit-code changes (scripts depend on them), board revisions supported, template upgrades, and what was
  tested on which board.
- Pure refactors and typo fixes don't need an entry.
- On a release, rename `[Unreleased]` to the version and date and start a new empty `[Unreleased]`.

The tpm repo's `/sync-repos` reads this file to log progress, so a missing entry means work nobody sees.

## Layout

- Cargo workspace (root `Cargo.toml`, one shared version):
  - `donguru-cli/` — the `donguru` binary (also installed as `dg`): clap command tree (`src/cli.rs`), one module
    per command group in `src/commands/`, exit codes (`src/exit.rs`), colour theme (`src/theme.rs`).
  - `donguru-core/` — host library: device enumeration and selection, the typed API used by the CLI and future
    GUI / Python bindings, `thiserror` errors.
  - `donguru_api/` — `#![no_std]` WireWeaver API crate (`DonguruApi`), shared by firmware and host. Its name and
    version identify the API on the wire.
- `fw/donguru/` — firmware (embassy, thumbv6m), a separate Cargo project with its own `Cargo.lock` and toolchain:
  `src/main.rs` (clocks, tasks, fault handlers), `led.rs`, `adc.rs`, `button.rs`, `ww.rs` (API server), `init.rs`.
- `docs/` — documentation site (Zensical), published to vhrdtech.github.io/typec_dongle. `developer-notes.md` —
  CLI implementation notes.

Logic belongs in `donguru-core`, not in the CLI: commands parse arguments, call core and render the result, so the
GUI and Python bindings can reuse the same code.

## Commands

The `justfile` is the single entry point for tasks; CI runs the same targets.

```sh
just                     # list recipes
just verify              # fmt check, clippy (-D warnings), tests: what CI runs
just fmt
just hooks-install       # once after cloning: prek pre-commit / pre-push hooks
just serve-docs          # docs site with live reload
just fw-size             # firmware release build, FLASH / RAM use, biggest symbols, counters

cd fw/donguru && cargo run              # flash and run with probe-rs, defmt logs over RTT
cd fw/donguru && cargo run --release
ww list / ww introspect                 # with the dongle on USB (wire_weaver_cli)
```

Before declaring a change done: `just verify` passes, the firmware builds (dev and release) when you touched it,
and hardware changes were tested on a board, or you say clearly that they weren't and what to check.

## Conventions

- **CLI**: follow [docs/design/cli/](docs/design/cli/index.md): `noun verb` commands, global `--device`,
  `--format text|json|jsonl`, `--color`, `-v` / `-q`. stdout carries only data, diagnostics go to stderr. Exit codes
  are a public contract (`src/exit.rs`): keep them stable and documented once shipped.
- **Firmware API changes**: edit `DonguruApi` in `donguru_api/src/lib.rs`, implement it in `fw/donguru/src/ww.rs`,
  bump the `donguru_api` version, and follow WireWeaver's `docs/evolution/rules.md`. Update the CLI in the same
  piece of work, or add a FEATURES.md item for it.
- **Firmware**: no panics on host input; count unexpected events with `cnt!` (and `bkp_cnt!` for ones that must
  survive a reset); watch flash and RAM with `just fw-size` (512K flash, Cortex-M0+: avoid pulling in 64-bit
  division and float formatting). `time-driver-tim4` is on purpose: TIM15 drives the PF1 LED.
- **Electrical safety**: never switch power or data lines in a way that can damage the host or the device under
  test (OVP, hot-switching). Check the schematic in `hw/b153_typec_serial` when in doubt and say what you assumed.
- No `unwrap`/`expect` on data from the device, files or the user in host code.

## Tests

- Host crates: unit tests next to the code, snapshot tests with insta for CLI output once commands print real
  data. Run through `just test`.
- Device access goes through a trait in `donguru-core` so commands can be tested with a mock transport
  (FEATURES.md CORE-3).
- Firmware logic that doesn't need hardware (button decoding, LED animation, conversions) can move to a module that
  also builds for the host, with unit tests. Hardware behaviour is checked on a board and recorded in the CHANGELOG
  entry.

## Commits

Conventional Commits with a scope: `feat(cli): ...`, `fix(fw): ...`, `feat(api): ...`, `feat(core): ...`,
`docs: ...`, `build: ...`, `ci: ...`. Short imperative summary, blank line, body with what and why; reference
feature IDs (`feat(cli): gpio read over WireWeaver (CLI-5)`). The pre-commit hooks must pass; don't skip them.

Never commit on your own initiative. When a change is done, update FEATURES.md and CHANGELOG.md, then show the
proposed commit message and the files to stage, and ask. Approval covers that one commit only. Never push; the
user's tooling does that.

## Versions

Every commit with real work bumps the version in the same commit (manifest + CHANGELOG entry), so any build
traces back to a commit:
- Patch for fixes and small changes, minor for features or anything breaking before 1.0, major only when the
  owner says so. In a workspace, only the crates that changed.
- Docs-only, CI-only and no-behaviour-change refactors skip it; a burst of follow-up fixes shares one bump.
- CLIs print version, git SHA and build time in `--version`, e.g. `tool 0.4.2 (a1b2c3d-dirty, built 3 Oct 2026
  18:20)`: a small `build.rs` without extra crates (`git rev-parse --short HEAD`, `-dirty` when
  `git status --porcelain` isn't empty, `rerun-if-changed` on `.git/HEAD` and `.git/index`, `unknown` without
  git). Firmware reports the same through `fw_info`. When touching a CLI that lacks it, add it.
