# Handoff: dap-512 (FW-19, P2534#13oct-1043)

## Done

- `donguru_dap/` (new workspace crate, no_std, 23 unit tests): `shape()` gives a CMSIS-DAP request's length and
  response bound from its own bytes, `Assembler` puts a request together from 64-byte packets, `response_needs_zlp`.
- `fw/donguru/src/cmsis_dap.rs`: 512-byte request / response buffers, the assembler in the read loop (select3:
  packet, `swd_release`, timeout), response written in 64-byte packets plus ZLP, 1 s response timeout.
- `fw/donguru/Cargo.toml`: `dap-rs` feature `usb-hs` (only switches the `DAP2_PACKET_SIZE` constant).
- FEATURES.md FW-19 (🧪), CHANGELOG `[Unreleased]`, AGENTS.md layout. `just verify` passes, firmware builds dev,
  release and release `--features plug-vbus-on`.

## Next

Waiting for the parent to say the bench is free (Donguru is the probe of the power-cut-bench sweep on omarchy-m1:
do NOT flash before that). Then on omarchy-m1 over ssh, J-Link `--probe 1366:0105`, chip `STM32G0B1CETx`, firmware
built with `--features plug-vbus-on` while the NRST bench is wired:

1. Before (firmware on main): `probe-rs read` of 64 KB of B129 flash at 1000 kHz (was ~7.7 s), and
   `donguru swd memtest --chip STM32G0B1CETx --size 16K --speed 1000`.
2. Flash this branch, repeat both, then the full RAM once. All patterns must pass.
3. Read the `dap_*` counters after (`cnt`): all should be 0.
4. Put the numbers in FEATURES.md FW-19 (🧪 -> ✅, board revisions) and CHANGELOG, `git rm HANDOFF.md`,
   `just test`, `just clippy`, `tpm work ready`.

## Decisions

- End of request by parsing the command length, not by an inter-packet timeout: reasoning in the module doc of
  `donguru_dap/src/lib.rs`. Timeouts only for resync (100 ms) and unknown commands (10 ms).
- No ZLP after a response of exactly 512 bytes (hosts that read exactly the DAP packet size); probe-rs never asks
  for one that long.
- Dropped requests get no response: the host times out (1 s in probe-rs).

## Gotchas

- This repo has no `just lint`: `just clippy` / `just verify` are the lint recipes.
- The firmware crate is not formatted by `cargo fmt` (120 columns by hand, no rustfmt.toml); the workspace crates
  are (100 columns).
- embassy-stm32's endpoint `write` returns once the packet is loaded, not once the host took it.
