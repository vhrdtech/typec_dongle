# Changelog

All notable changes to Donguru: firmware, API crate, host library and CLI. The format follows
[Keep a Changelog](https://keepachangelog.com/), versions follow [Semantic Versioning](https://semver.org/). Feature
IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

## [0.10.0] - 2026-10-15

### Changed

- Release firmware is built for speed (`opt-level = 3`) instead of size: with the single-buffered USB endpoints of
  embassy-stm32 0.6, every microsecond spent per packet makes the host retry, so "z" cost ~40%. Release build,
  `ww usb-loopback` with this branch's wire_weaver: 1 KB messages 123 -> 214 round trips/s, 258 -> 419 kB/s host ->
  device, 307 -> 470 kB/s device -> host; 64 B messages 584 -> 2400 round trips/s. Flash 87 -> 113 KB of 510 KB
  (FW-24). Tested on B153A.

### Fixed

- Firmware runs with the flash prefetch on: embassy-stm32 0.6 left it off at 64 MHz, so code ran 8-16% slower;
  WireWeaver over USB 1 KB messages (`ww usb-loopback`) 373 -> 399 kB/s host -> device, 317 -> 451 kB/s device ->
  host (FW-24). Tested on B153A.

## [0.9.0] - 2026-10-15

### Changed

- Builds against wire_weaver master again, where API enums get a variant for values from a newer version (SW-47),
  `donguru_api` 0.1.4, same bytes on the wire, older CLIs keep working:
  - `BoardVariant` keeps its own `Unknown` (hub PID matched no part); a variant from a newer firmware reads as
    `BoardVariant::Other(n)` (`future_version = Other`): new boards will add variants (API-7).
  - `SwdOrientation` is `no_future_version`: a Type-C plug has two orientations, the set is complete (API-4).
  - `swd_mode`: a mode from a newer host arrives as `SwdMode::Unknown` and the write is refused, the current mode
    stays (counter `swd_mode_unknown`) (API-5).

### Fixed

- Firmware: flip-link stack overflow protection is really in effect: `linker = "flip-link"` in
  `fw/donguru/.cargo/config.toml` (embedded_bedrock ROB-12). The template's `cargo:rustc-linker` line in `build.rs`
  is no build-script directive, so the firmware linked without it and the stack sat above `.data` / `.bss`. Now the
  stack runs from below `.data` down to the start of RAM, checked in the ELFs of dev, release and both examples;
  not yet on a board (FW-1).

## [0.8.0] - 2026-10-14

### Added

- Firmware: board variant (Main, 2-port USB2512B; Plus, 4-port USB2514B) detected at boot from a GPIO strap
  (PA10/R902, `donguru_hub` crate) instead of a hardcoded constant, and the hub's config built per detected
  variant; the hub's own power-on PID is not usable for this (reads 0 before configuring, regardless of delay —
  see Fixed below for the readback this corrects) (FW-22).
- API: `board_variant` read-only property (`BoardVariant::Main` / `Plus` / `Unknown`), `donguru_api` 0.1.3;
  additive, older CLIs keep working (API-7).

### Fixed

- Firmware/docs: the hub's VID/PID/DID registers do not hold a factory default readable before configuring on
  this board (hw_b153a.md §9's "defaults read 0" caution confirmed, not a delay or SMBus-framing issue); the
  0.7.0 entry below only fixed the readback *after* the config write (HUB-5).

## [0.7.0] - 2026-10-14

### Changed

- Firmware: hub (U301) bring-up now goes through the `usb251xb` crate (typed config, MIT/Apache, pure Rust)
  instead of raw SMBus bytes: same VID/PID/DID and config bytes as before, plus the port 2 and port 4 D+/D-
  polarity swap (fixes a full/high-speed device on the plug looking low-speed to the hub), and a VID/PID
  readback logged right after configuring, using the crate's correct SMBus block-read framing instead of the
  plain register read that used to come back as 0 (`src/hub.rs`, FW-6, FW-18 recovery unchanged on top). Tested
  14 Oct 2026 on B153A (J-Link via omarchy-m1): HUB-1 (config/attach) and HUB-5 (VID/PID readback, now
  `0424/2512` instead of 0) confirmed; HUB-7 (the port 2/4 swap itself, with a device on the plug) not yet.

### Fixed

- Firmware: a reMarkable 2 behind the hub's plug port showing up as a low-speed device with descriptor error -32
  (FW-6, port swap above). Built, not yet tested on hardware (HUB-7).

## [0.6.0] - 2026-10-13

### Added

- Firmware: SPI-driven SWD as a second SWD-host mode, SPI2 through the SBU level shifter (`SBU_OE` high while it
  is in use, low again on every release), up to 4 MHz in steps of 64 MHz / 2^n; experimental, bit-bang stays the
  default. Normal cable orientation only. Selected with the new `swd_mode` property or the firmware feature
  `swd-spi` (FW-21). Tested 13 Oct 2026 on B153A + B129A: reads (bytes identical) and memtest pass at 100 / 1000 /
  2000 / 4000 kHz, 8000 kHz fails, pins Hi-Z after release; highs through the level shifter are weak against the
  target's pull-downs (about 2.0-2.5 V at B129A), and it is no faster than the bit-bang there.
- API: `swd_mode` property (`SwdMode::BitBang` / `SwdMode::Spi`), `donguru_api` 0.1.2; additive, older CLIs keep
  working (API-5).

### Changed

- Firmware: the bit-banged SWD clock runs at the requested frequency instead of well under it (about 400 kHz at
  a requested 1 MHz before), up to about 3 MHz, with both clock halves the same length; requests up to 3 MHz are
  accepted (cap was 1 MHz). A DUT with series resistors and clamp diodes on SWD limits the usable clock (B129A:
  2 MHz or less). Tight assembly bit loops on the GPIO registers, timed against SysTick at boot (`src/swd.rs`,
  `donguru_dap::swclk`, FW-20). `donguru_api` unchanged (0.1.1). Tested 13 Oct 2026 on B153A + B129A: 64 KB flash read 7.03 s -> 1.77 s
  at 1000 kHz and 1.07 s at 4000 kHz (bytes identical), `donguru swd memtest` 16K at 400 / 1000 / 2000 / 4000 kHz
  and the full RAM at 4000 kHz pass (72 s, was 493 s at 1000 kHz), pins Hi-Z after disconnect; that build ran
  the top speed at 70 % duty, fixed since (equal halves, cap 3 MHz): re-measured 0.98 / 2.00 / 2.78 MHz at
  1000 / 2000 / 3000 kHz asked, 47-52 % duty, 64 KB read 1.19 s at 3000 kHz, memtest 16K passes up to 3000 kHz.

## [0.5.0] - 2026-10-13

### Changed

- Firmware: the CMSIS-DAP probe reports a 512-byte DAP packet instead of 64, so probe-rs moves 126 words per
  `DAP_TransferBlock` instead of 15; requests and responses span several 64-byte Full-Speed USB packets
  (`src/cmsis_dap.rs`, new crate `donguru_dap` with the framing rules and their unit tests, FW-19). `donguru_api`
  unchanged (0.1.1), old CLIs keep working. Tested 13 Oct 2026 on B153A + B129A with probe-rs 0.32: 64 KB flash
  read at 1000 kHz 7.7 s -> 7.05 s (bytes identical), `donguru swd memtest` 16K at 100 / 400 / 1000 kHz and the
  full RAM at 1000 kHz pass, pins Hi-Z after disconnect. Only about 9 % faster: the bit-banged SWD clock, not
  the USB round trips, limits the speed.

### Fixed

- Firmware: a truncated CMSIS-DAP request (shorter than its command needs), or one asking for a response larger
  than the DAP packet, is dropped and counted instead of panicking inside `dap_rs`; a host that stops reading a
  response no longer blocks `swd_release` (FW-19).

## [0.4.1] - 2026-10-12

### Fixed

- Firmware: Donguru no longer stays off USB for good after a brown-out on the hub (U301) that leaves the MCU
  running — a hard turn-on load on the header's switched VBUS (Q102) is one way to trigger it. The hub shares
  `+3V3_DCDC` with the MCU and loses its SMBus config and attach state when its rail sags enough to reset it; a
  watchdog now notices (the composite USB device losing the configured state it had reached, backed by a periodic
  I2C probe) and redoes the reset + config + attach bring-up (`src/hub.rs`, FW-18). Tested 12 Oct 2026 on B153A: a
  debugger-forced hub reset (PC13 pulsed low/high via J-Link, standing in for the brown-out since the bench's new
  10R VBUS_SW resistor stops that from reproducing) recovered 10/10 times, about 2 s each.

## [0.4.0] - 2026-10-12

### Added

- CLI: `donguru swd memtest`, an integrity check of an SWD link: memtest-style patterns (data and address bus,
  solid, checkerboard, address-in-address, moving inversions, byte lanes, seeded random) written to the target's
  RAM through the probe and read back, at one or more SWD speeds; the RAM is restored and the target resumed after
  (CLI-18). The patterns live in `donguru-core` (`swd_memtest`, feature `probe-rs`) (CORE-5).

## [0.3.0] - 2026-10-12

### Added

- Firmware: plug CC drive for SWD-host, stopgap and hardcoded (PD0/PD2 push-pull high on `DAP_Connect`, Hi-Z with
  SWCLK/SWDIO on `DAP_Disconnect` / endpoint disable / `swd_release`), so a downstream board gating its SBU->SWD
  mux on CC (e.g. B129A) switches over during a probe-rs session (FW-17). Built, not tested on hardware.
- Firmware: `plug-vbus-on` build feature, off by default: AOZ1375 on from boot so one cable powers the DUT and
  carries SWD, a stopgap until FW-8. Never on the P2625 power-cut bench (the DUT runs from Q102 there).

### Fixed

- Firmware: the USB hub is configured and attached at boot (`src/hub.rs`, from the FW-5 bring-up example). Until now
  only `examples/usb_serial.rs` did it, so the main firmware never showed up on USB: the MCU sits behind the hub.
- Firmware: SWD-host drives SWCLK as an output on every clocking path. It was set only by `DAP_SWJ_Pins`, which
  probe-rs doesn't send, so SWCLK stayed an input and no target ever acknowledged (FW-15).
- Firmware: SWD-host samples SWDIO before the rising SWCLK edge (as CMSIS-DAP `SW_READ_BIT`), not right after it
  when the target is changing it (FW-15).
- Firmware: SWD-host on PA1 (SBU1) / PA0 (SBU2) with V_SBU at 3.3 V, not PB8/PB6: R211/R212, the level shifter
  bypass, are not fitted on B153A, so PB8/PB6 never reached the plug (FW-15).

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
