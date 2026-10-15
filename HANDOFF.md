# Handoff: api-tree (API-8, P2534#15oct-2043)

## Done

- `donguru_api` 0.2.0: whole v1 tree declared (design: tpm `prj/P2534-donguru/design/api-v1.md`, "As built in API-8"
  has the decisions taken here).
- Firmware: `src/ww.rs` split into `src/ww/` (mod.rs = plumbing, one module per level). Moved items implemented,
  `debug.led_set` behind `debug-api` (default on), consts minimal, everything else `Unimplemented`. Observe wired in
  the server task. Fixed endpoints EP1/EP2 (WireWeaver) and EP3 (CMSIS-DAP).
- Flashed (release) on B153A via J-Link: `ww list`, `ww introspect`, Python r/w of moved items, observe of
  `plug.orientation` with a generated client all pass. `just verify` green, fw dev + release build.
- Sizes and the per-level breakdown: CHANGELOG [Unreleased] and the design doc's budget section.

## Open

- CMSIS-DAP `probe-rs read` of B129A with `plug.orientation`: "no target responded" in both orientations, the DUT
  seems unpowered. Parent asked Roman whether a `plug-vbus-on` build is safe (B129A maybe also fed from header
  VBUS_SW). Do not flash plug-vbus-on without that answer. Command once powered:
  `probe-rs read --probe c0de:cafe --chip STM32G0B1CETx --speed 1000 b32 0x1FFF7590 3`, orientation set from
  Python (`dev.plug.orientation.write("Swapped")`).
- `ww info` ends with "Called a method that required event loop to be running": `ww_report::Contract` disconnects on
  drop, then `ww`'s main disconnects again. wire_weaver bug, reported to the parent, not fixed here.

## Gotchas

- wire_weaver worktree is at master `1a04005` (no own commits).
- Python module for tests: `uvx maturin develop --release --no-default-features` in `wire_weaver_py` into a venv.
- A generated host client needs every stdlib trait crate as a direct dependency.
