# Handoff: cli-power (CLI-11, CLI-6, CLI-16)

Session `cli-power` on `gpd-omarchy`, task P2534#15oct-2044i. Branch `s/16oct2026-cli-power`, worktree
`/tmp/wt/cli-power/typec_dongle`.

## Done

- `donguru-core/src/vbus.rs`: `force_on(max_voltage_v)` / `auto(max_voltage_v, max_current_a)` build a `VbusMode`
  from plain f64 volts/amps (`max_power` is the voltage × current product, so it never trips before the voltage or
  current limit already would). `DEFAULT_MAX_VOLTAGE_V = 5.5`. Unit-tested.
- `dg power status / on / off / cycle / meter / watch / totals` (CLI-11), `donguru-cli/src/commands/power.rs`:
  - `status`: plug + header + meter in one read/table (text and json/jsonl).
  - `on` / `off`: `--allow-voltage` / `--trip` build the mode via `donguru_core::vbus`; after the write, re-reads
    `plug.vbus` and the command's exit code reflects whether VBUS actually ended up `On` / `Off` — a trip or a
    latched fault is a scriptable failure, not a silently "successful" call.
  - `cycle --off --release-sbu`, `meter`, `totals [--reset]` (charge/energy shown as mAh/mWh), `watch
    [--interval | --on-change]` (observes `meter.reading` through CLT-28, Ctrl-C to stop; `--format json` is
    refused since it can't represent a stream — use text or jsonl).
- `dg usb status / connect / disconnect` (CLI-6), `donguru-cli/src/commands/usb.rs`: rewritten from scratch —
  the old `usb list/attach/detach/power/data` stub modeled per-port hub control, which the current API tree
  doesn't have (v1 has no per-port hub switch, see API-11's note). Now just `plug.data`.
- `dg header vbus status / on / off`, `dg header relay status / on / off` (CLI-16), new
  `donguru-cli/src/commands/header.rs`: `vbus on` always prints the present receptacle voltage (from
  `meter.reading`, falls back to a plain warning if the meter isn't implemented yet) and the "not a regulated 5 V
  supply" warning; `--allow-voltage` is the explicit confirmation needed above 5.5 V. Shares `VbusOnArgs`,
  `mode_from_args`, the `fmt_vbus_state` / `fmt_vbus_mode` / `*_json` renderers and `report_vbus_state` with
  `power.rs` (`pub(crate)` there).
- Shared CLI plumbing moved into `donguru-cli/src/commands/mod.rs`: `runtime()` (was private to `device.rs`),
  `selector()`, `with_dongle()` (open → run an async closure → close, mapping to `anyhow::Result`), `optional()`
  (`Unimplemented` → `Ok(None)` instead of failing the command). `device.rs` now uses `with_dongle` too.
- `donguru-cli/Cargo.toml`: tokio gained the `signal` feature (Ctrl-C in `power watch`).
- Tests: unit tests in each new/changed command module against `donguru_core::mock::MockDevice` (status
  rendering, a real `--allow-voltage`/`--trip` → `VbusMode` mapping, a trip reported as a CLI failure, usb
  connect/disconnect round-trip, header vbus + relay round-trip). `just verify` passes (fmt, clippy -D warnings,
  full workspace test suite).
- Docs: `FEATURES.md` CLI-6/CLI-11/CLI-16 updated with pointers and board-test status (🧪, not yet run on
  hardware); `CHANGELOG.md` `[Unreleased]`; `docs/user/usage.md` filled in with power/usb/header examples (was a
  stub).
- Filed `CORE-6` (🐛, not fixed): the mock's VBUS guard (`donguru-core/src/mock/mod.rs`'s `MockState::apply`)
  always applies a mode write and reports `Tripped(OverVoltage)` if already over the limit, instead of refusing
  the write as `docs/user/power.md` *and* the real firmware-logic crate `donguru_power`
  (`Switch::set_mode`/`refused_over_the_limit_changes_nothing`) both say it should. The CLI copes either way
  (it re-reads `vbus` after any write), but a future CORE-3 test or another command that assumes `Set` means
  "nothing changed" would be testing the wrong thing against the mock. Left for CORE-3 to pick up — didn't touch
  the shared mock module from this session.

## Not done / needs the board

Nothing in this piece can be tested without the B153A: every command above is snapshot/mock-tested only. When the
board is back on omarchy-m1 (coordinate with other sessions queued for it via `tpm work say`):

- Run each of the new commands once against a real dongle: `dg power status/on/off/cycle/meter/watch/totals`,
  `dg usb status/connect/disconnect`, `dg header vbus status/on/off`, `dg header relay status/on/off`.
- `power meter` / `power watch` / `power totals` will answer `Unimplemented` until session `power-meter` (API-12)
  lands the firmware meter — that's expected, not a bug in this piece.
- `power cycle --release-sbu` is meant to replace raw `ww` calls in the P2625 bench recipe (task's "Done"
  criterion) — once it's confirmed working on hardware, point that recipe at it and say so to whoever owns P2625.
- Mark CLI-6 / CLI-11 / CLI-16 ✅ in FEATURES.md (with the board revision) once tested, instead of 🧪.

## Gotchas for whoever picks this up

- `VbusMode`/`VbusState`/`GuardTrip` all carry a hidden `Unknown(_)` variant from `derive_shrink_wrap` (for a
  newer firmware's future variants) — every `match` over them needs that arm, the compiler will tell you.
- `ww_si`'s numeric constructors (`Volt::volts_f32`, `Ampere::amperes_f32`, …) are reachable as inherent methods
  through `donguru_core::api::Volt` etc. without adding `ww_si` as a direct dependency anywhere outside
  `donguru-core` — Cargo's feature unification means the `f32`/`electrical` features `donguru-core` turns on
  apply workspace-wide. Keep new SI-value-building logic in `donguru-core` (this session added `vbus.rs` for
  that reason) rather than reaching for `ww_si` from the CLI directly.
- `Observer::recv_event()` returns `wire_weaver_client::StreamError`, not `donguru_core::Error` — don't try to
  `?` it into a `donguru_core::Result` block, match it explicitly (see `power::watch`).
