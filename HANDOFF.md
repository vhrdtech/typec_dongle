# Handoff: py-bindings (CORE-4), paused 24 Oct 2026

Paused by the user (`tpm work pause`) mid-step. State on branch `s/24oct2026-py-bindings`:

## Done

- `feat(core)`: `Selector::Label` (`label:L`, and plain text that matches no serial falls back to an exact label, any case), with unit test `select_by_label`; FEATURES CORE-1, CHANGELOG, `docs/design/cli/index.md` updated.
- WIP, not built yet: `donguru-py/` crate (PyO3 0.29, abi3-py310, lib name `_donguru`, `[lib] test = false`). It is **not** in the workspace `members` yet so the workspace keeps building; add `"donguru-py"` back to `Cargo.toml` members when resuming.
  - `src/lib.rs`: global tokio runtime, `block_on` with `py.detach`, `Shared { RwLock<Option<Dongle>> }`, `list()`, `open(selector)`, `Dongle` class (entry, serial, info, firmware_version, label get/set, level getters, close, context manager).
  - `src/errors.rs`: `donguru.Error` + NoDevice, DeviceError (`.kind`, `.item`), Unimplemented, Timeout, Incompatible, LinkError.
  - `src/values.rs`: DeviceInfo, Info, Reading (V/A/W, t), Totals (C/J/A/s, charge_mah, energy_wh), VbusStatus, VbusMode (`off()/on()/auto()`), Deadband, UartConfig.

## Next

1. `src/levels.rs` (referenced from lib.rs, missing): Power (on(max_voltage, trip)/off/cycle(off, release_sbu)/state/mode), Meter (read, watch iterator via `observe` + `periodic`/`on_change`, check signals every ~100 ms; totals, reset_totals, temperature, window, deadband), Usb (connected get/set, connect, disconnect), Header (relay, vbus, vbus_mode, vbus_on, vbus_off), Uarts (`plug`/`header` → UartPort.configure(...) returning UartConfig), Settings (plug_vbus_at_boot, button_action, meter_window, meter_deadband, uart_plug, reset), Leds (identify); `register(m)`.
2. `src/mock.rs` (feature `mock`): `donguru.mock.MockDevice` (start, open, set_reading, set_totals, state getters).
3. Fix pyo3 0.29 warnings: `#[pyclass(skip_from_py_object)]` or `from_py_object` on the Clone classes (needed `from_py_object` for VbusMode, Deadband, UartConfig which are passed in).
4. lib.rs line 77/141 E0034 ("multiple applicable items"): return types of `#[pyfunction]` with `PyResult<Vec<..>>` — check after levels.rs exists.
5. `pyproject.toml` (maturin, `features = ["pyo3/extension-module", "mock"]`, python-source `python`), `python/donguru/__init__.py` + `.pyi` stub, `tests/test_mock.py` (pytest via `uv run`), `just py-test` / `py-build` recipes (plain commands).
6. Docs page `docs/user/python.md` (+ nav, scripting.md link), FEATURES CORE-4 🧪, CHANGELOG, decisions in tpm `prj/P2534-donguru/design/night-24oct-decisions.md` and `python-api.md` (#17oct-1638: blocking, SI floats, iterators; mock shipped in the wheel; power.on returns status instead of raising).
7. `just test --changed`, `just lint`, `git rm HANDOFF.md`, `tpm work ready`.
