# Changelog

All notable changes to Donguru: firmware, API crate, host library and CLI. The format follows
[Keep a Changelog](https://keepachangelog.com/), versions follow [Semantic Versioning](https://semver.org/). Feature
IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

## [0.2.0] - 2026-10-11

### Added

- AGENTS.md (repository rules), FEATURES.md and this changelog.
- Firmware: SWD-host over the plug SBU lines, CMSIS-DAP v2 (bulk) alongside WireWeaver on one composite USB
  device, `donguru_api` 0.1.1 (old CLIs still work: additive, same major/minor): `swd_orientation` rw property
  and `swd_release` Hi-Z disconnect, for the P2625 power-cut bench (FW-15, API-4). The pins also go Hi-Z on a
  clean `DAP_Disconnect` and on a USB reset/suspend of the CMSIS-DAP interface, not only on an explicit
  `swd_release` call, so a bench harness only has to end the probe-rs session before cutting power. Built, not
  tested on hardware.
- CLI: stub `swd orientation` / `swd release` commands, same "not implemented" state as the other device commands
  pending CORE-1 (CLI-17).

### Changed

- `donguru_api` 0.1.0 -> 0.1.1 (wire-compatible addition, see above).

### Fixed

- Firmware 0.1.1: input current sign, positive now means charging the DUT (computer to DUT), negative the DUT
  feeding the computer. The INA181 on B153A has IN+ on the plug side of the shunt (FW-3).

## [0.1.0] - 2026-10-01

Not released. Reconstructed from git history.

### Added

- Firmware: WireWeaver API over USB with `donguru_api` (`led_on` / `led_off`), template 0.5.0; built, not yet tested
  on hardware (API-1, API-2).
- Firmware: button and boot switch decoded from the PA7 ADC input, with edges and presses (FW-4).
- Firmware: ADC sampling with a DMA ring buffer, 64 MHz PLL clock; `just fw-size` (FW-3).
- Firmware: animated LEDs (FW-2); USB hub brought up on B153A (FW-5); project set up from the firmware template
  (FW-1).
- CLI: `donguru` / `dg` with the command tree, global flags, colour theme and completions
  (CLI-1 to CLI-3); `udev generate` (CLI-8); stubs for `info`, `device`, `usb`, `gpio`, `config` and external
  subcommands (CLI-4).
- Host library `donguru-core` skeleton: device selection types and errors (CORE-1, CORE-2).
- Documentation site with CLI and packaging design docs; CI workflow, justfile and pre-commit hooks (PKG-1, PKG-2).
