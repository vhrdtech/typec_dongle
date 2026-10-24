# Handoff: py-bindings (CORE-4), 25 Oct 2026

Resumed after the user's pause; handed off on tpm's context limit. State on branch `s/24oct2026-py-bindings`:

## Done

- `feat(core)`: `Selector::Label` (`label:L`, and plain text that matches no serial falls back to an exact label, any case), with unit test `select_by_label`; FEATURES CORE-1, CHANGELOG, `docs/design/cli/index.md` updated.
- `donguru-py/` crate (PyO3 0.29, abi3-py310, lib name `_donguru`, `[lib] test = false`), in the workspace members; `cargo check`/`clippy -D warnings --all-features` clean. Not yet built as a wheel nor run from Python.
  - `src/lib.rs`: global tokio runtime, `block_on` with `py.detach`, `Shared { RwLock<Option<Dongle>> }`, `list()`, `open(selector)`, `Dongle` class (entry, serial, info, firmware_version, label get/set, level getters, close, context manager).
  - `src/errors.rs`: `donguru.Error` + NoDevice, DeviceError (`.kind`, `.item`), Unimplemented, Timeout, Incompatible, LinkError.
  - `src/values.rs`: DeviceInfo, Info, Reading (V/A/W, t), Totals (C/J/A/s, charge_mah, energy_wh), VbusStatus, VbusMode (`off()/on()/auto()`), Deadband, UartConfig.

  - `src/levels.rs`: Power (on(max_voltage, trip)/off/cycle(off, release_sbu)/state/mode), Meter (read, watch → `MeterWatch` iterator with count/on_change/samples, Ctrl-C checked every 100 ms, close + context manager; totals, reset_totals, temperature, window, deadband), Usb (connected, connect, disconnect), Header (relay, vbus, vbus_mode, vbus_on, vbus_off), Uarts (`plug`/`header` → UartPort.configure(**kw)/apply(UartConfig)), Settings (plug_vbus_at_boot, button_action, meter_window, meter_deadband, uart_plug, reset), Leds (identify).
  - `src/mock.rs` (feature `mock`): class `MockDevice(path=None)` registered in `_donguru` (to re-export as `donguru.mock.MockDevice`): open, set_reading, set_totals, getters plug_vbus_mode, vbus_cycles, data_connected, relay, identified.

## Next

1. `donguru-py/pyproject.toml` (build-backend maturin, `[tool.maturin] features = ["pyo3/extension-module", "mock"]`, `python-source = "python"`, `module-name = "donguru._donguru"`, dev group pytest), `python/donguru/__init__.py` (`from ._donguru import *`, `__version__`, `__all__`), `python/donguru/mock.py` (`from ._donguru import MockDevice`), `_donguru.pyi` stub + `py.typed`.
2. `donguru-py/tests/test_mock.py`: open the mock, info/firmware_version/label, power on/off/cycle/trip, meter read + watch(count=3) + totals, usb, header relay/vbus, uart.header.configure, settings round-trip, errors (DeviceError.kind, closed dongle -> LinkError), `open("label:x")` -> NoDevice. Run: `cd donguru-py && uv run pytest` (uv builds the package with maturin; check network/maturin availability).
3. justfile recipes `py-test` (`uv run --project donguru-py pytest donguru-py/tests`) and `py-build` (`uv build --wheel donguru-py`), plain commands only. Maybe call py-test from CI later (note in FEATURES).
4. Docs page `docs/user/python.md` (+ zensical nav, link from `docs/user/scripting.md`), FEATURES CORE-4 🧪 (no hardware tonight: list what to check on B153A — list/open by label, power on/off/cycle, meter watch rates, uart configure), CHANGELOG Added (CORE-4).
5. tpm decisions: `prj/P2534-donguru/design/python-api.md` (#17oct-1638 shape: blocking, SI floats, iterators, strings for enums, exceptions per category) and lines in `night-24oct-decisions.md` (blocking first; mock shipped in the wheel under `donguru.mock`; `power.on` returns `VbusStatus` instead of raising; pandas `record()`, `usb.cycle(wait_for)`, `inputs.events()`, `measure()` left for later).
6. `tpm work rebase` onto main (moved to d5bafc5, v0.20.29), `just test --changed`, `just lint`, `git rm HANDOFF.md`, `tpm work ready --note ...`, `tpm work say parent` (3 lines), then per the user's queue message: `tpm work resume pkg@gpd-omarchy` and end.

## Gotchas

- `cargo` may not be on PATH in a fresh shell: `export PATH=$HOME/.cargo/bin:$PATH`.
- pyo3 0.29: Clone pyclasses need `from_py_object` (taken as arguments) or `skip_from_py_object`; `py.detach` (not allow_threads); closures passed to detach must be Send, so lock Mutexes inside them.
- Values to the device go as integer milli units (`values::milli`): the firmware's ww_si has no floats.
