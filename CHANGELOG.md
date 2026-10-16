# Changelog

All notable changes to Donguru: firmware, API crate, host library and CLI. The format follows
[Keep a Changelog](https://keepachangelog.com/), versions follow [Semantic Versioning](https://semver.org/). Feature
IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

## [0.20.1] - 2026-10-16

### Fixed

- `dg device counters` and `dg device log` find the firmware ELF in the shared directory of `~/.config/ww/config.toml` `[elf] dirs` (the ww-elf share, CORE-7): donguru-core needs cnt_core 0.5.1, which reads it.

## [0.20.0] - 2026-10-16

### Added

- `just fw-size` now stores the release build's ELF in the ELF cache (`cnt elf add`) after building (CORE-7), so
  `dg device counters` / `dg device log` can find it by build id later without `--elf`.
- `cnt` 0.5.1 (upstream, not a typec_dongle change, CORE-7): `cnt run`, the cargo runner `fw/donguru` flashes
  with, now caches the flashed ELF by build id the same way; `cnt_core::elf` reads a config file
  (`~/.config/ww/config.toml`'s `[elf] dirs`) for a directory shared between machines, in addition to
  `WW_ELF_PATH`, so a board flashed on one dev PC is found by `dg` on another. That directory is declared as the
  `ww-elf` Syncthing share between gpd-omarchy and omarchy-m1 (`tpm area/ops/provision.toml`), approved and
  syncing. Tested 16 Oct 2026 on B153A rev A: flashed via `cargo run`, `dg device counters` / `dg device log`
  found the ELF with no `--elf` and decoded it (against `cnt_core` patched to the unreleased branch; works
  against crates.io once `cnt` 0.5.1 is out).

## [0.19.1] - 2026-10-16

### Fixed

- The RTT log never blocks the firmware (FW-27): with probe-rs attached, a burst of logging used to wait for the
  probe inside the logger (probe-rs sets the channel to blocking), holding up other tasks. Now each line goes to RTT
  whole or is skipped there, whatever mode the probe set; the USB log (`device.log`) still gets it. A line costs
  ~62 us instead of ~620 us with probe-rs attached. 512 B more RAM. Needs wire_weaver `ww_defmt_logger` with the
  non-blocking tee (STD-26 fix). Tested 16 Oct 2026 on B153A: floods of 1000 lines with and without probe-rs, exact
  lost counts, no ADC overrun.
- `debug.log_flood` yields between lines, so other tasks run during the flood (it overran the ADC with the probe
  fixed too); `device_log --flood` logs one more line after it, which carries the count of the lines lost at the end
  (a loss is reported with the next line).

## [0.19.0] - 2026-10-16

### Added

- `dg power status / on / off / cycle / meter / watch / totals` (CLI-11): a combined plug + header + meter table,
  the VBUS modes with `--allow-voltage` / `--trip`, `cycle --off --release-sbu`, one meter reading, charge/energy
  totals in mAh/mWh with `--reset`, and `watch` streaming `meter.reading` through observe until Ctrl-C. `on` / `off`
  / `cycle` exit non-zero unless VBUS actually ended up where asked.
- `dg usb status / connect / disconnect` (CLI-6): the plug's USB data switch (U701), replacing the earlier stub's
  port-based tree (no per-port hub control in v1).
- `dg header vbus status / on / off` and `dg header relay status / on / off` (CLI-16): `vbus on` always prints the
  present receptacle voltage and a warning that VBUS_SW is the computer's own VBUS, not a regulated 5 V supply;
  `--allow-voltage` is the explicit confirmation needed above 5.5 V.
- `donguru_core::vbus`: builds a `VbusMode` from plain volts/amps for the CLI, the GUI and Python, as integer
  millivolts/milliamps/milliwatts and (`power cycle`'s `off`) milliseconds: the firmware's `ww_si` has no `f32`, so
  a float-encoded value doesn't decode there at all (`PropertyDesFailed` / `ArgsDesFailed`, found and fixed testing
  on B153A).
- Tested 16 Oct 2026 on B153A rev A, no DUT: `power status/on/off/cycle/meter/watch/totals`, `usb
  status/connect/disconnect`, `header vbus status/on/off`, `header relay status/on/off`, text/json/jsonl. Plug
  VBUS switches and the meter confirms the draw (~16 mA); `header vbus on`'s warning shows the real measured
  voltage. Not tested: a trip or `Auto`'s current/power limits under load (needs a DUT).

## [0.18.0] - 2026-10-16

### Added

- `dg info` and `dg device select` (CLI-7): `info` is `device info`'s contract summary (firmware, board/variant,
  uid, label, last reset) under the short, no-noun spelling; `select SELECTOR` resolves against the connected
  dongles and pins the serial in `./donguru.toml`, read as the lowest-priority `--device` fallback.
- `dg device label [NAME] [--notes]`, `dg device reset-cause [--clear]`, `dg device counters [--watch]` and
  `dg device log [--follow]` (CLI-19): the `UserInfo` name/notes; `LastReset`'s cause, message, boot count and
  crash record with `clear`; `ww_counters` and the stored `ww_log` defmt frames, decoded with the firmware's ELF
  found by its build id (`cnt_core`, `defmt-decoder`, the same mechanism `ww counters` / `ww report` use).
- `donguru_api` unchanged at 0.2.0; `donguru-core` gains `Device::notes`/`set_notes`, `boot_count`,
  `clear_last_reset`, `counters`, `log_dump`, `log_follow` and the `counters`/`log` modules behind them.
- The v1 CLI command tree, written into `docs/design/cli/index.md` as the spec (no open questions left there).
- Tested 16 Oct 2026 on B153A rev A (text and json, exit codes): `info`, `device info/select/label/reset-cause`
  all work end to end, including a real decoded crash record (`HardFault`) cleared by `reset-cause --clear`;
  `device log` takes the "nothing yet" path (API-10 not built); `device counters` reaches the real ELF-by-build-id
  lookup and fails with the documented message since no firmware ELF was cached for this board's build id — the
  happy decode path still needs a firmware build with its ELF kept around.

### Changed

- The mock (CORE-3) now models `device.last_reset` / `boot_count` / `clear` and `device.label.notes`
  (`notes_max` was 0, now 256 to match the firmware); `device.counters` / `device.log` still answer
  `Unimplemented` for their own RPCs, as the firmware does until API-10 and more counters land.
- `donguru` binary no longer duplicates the CLI's module tree; it uses the `donguru_cli` library crate directly.

## [0.17.0] - 2026-10-16

### Added

- defmt log over WireWeaver (API-10; `donguru_api` 0.2.0 -> 0.2.1, additive, 0.2.0 hosts still connect):
  `device.log.frames` carries the firmware's defmt log, whole frames, with the exact number of lines dropped;
  `device.log.DEFMT` gives the encoding and the build id to find the ELF. The log is kept in 8 KiB of RAM that
  survives a soft reset, from boot on: a host that resumes the stream gets the lines from before it connected and from
  before a crash (not the fault handler's own line; not across a power cut). RTT still shows the log with a probe
  attached. Test tool: `cargo run -p donguru-core --example device_log -- --elf ...` (`--flood N`, `--no-read`).
  Tested 16 Oct 2026 on B153A: boot log over USB and RTT at once; a flood of 1000 lines gives 227 lines and
  "773 lost" (RTT, with probe-rs blocking, all 1000); lines from before a HardFault forced through the J-Link arrive
  after the reset. Known: an ADC overrun while writing to a host that died (FW-26).
- `debug.log_flood(count)`: logs `count` lines at once to check the lost count (API-10).

### Changed

- Firmware: defmt's logger is `ww_defmt_logger` instead of `defmt-rtt` (API-10). The RTT channel `defmt` is now set
  up by `rtt-target`, NoBlockSkip as before; a probe that switches it to blocking (probe-rs does) makes log calls
  wait for it: a burst of logging then holds up other tasks (an ADC overrun during a flood of 1000 lines).
- Firmware size, release build, `just fw-size`: FLASH 276.8 -> 297.7 KiB of 510 KiB, RAM 16.7 -> 33.9 KiB (API-10:
  the 8 KiB store and its 128 B header slots, the logger's 4 KiB ring and 1 KiB packing buffers, the reliable stream
  sender and its CRCs).

## [0.16.0] - 2026-10-16

### Added

- Power meter in SI units (API-12, FW-3): `meter.reading` (VBUS_IN, current towards the DUT, power, one ADC window),
  `temperature`, `totals` (charge, energy, peak current, since) with `reset_totals`, `window` (default 10 ms),
  `deadband` (default 20 mV / 10 mA; `OnChange` observers are marked only beyond it, `totals` once a second),
  `calibration` (nominal until FW-9). Integer milli values, no host change needed: API items existed since 0.2.0.
- `adc` (`ww_adc::Adc`): channels `vbus_in`, `current`, `vdda`, `temperature`, `button`, `io9`, `io7`, `io10` with
  raw code and value in milli units; `sbu1` / `sbu2` answer `Busy` for now.
- Tested 15 Oct 2026 on B153A without DUT power (needs API-11): values plausible, totals integrate, `OnChange`
  observers quiet on an idle DUT. Under load (B129A on the plug, ForceOn 5.5 V, 16 Oct): 16 mA with a 6 mA idle offset, totals 16.4 mC/s, peak
  39 mA at switch-on, observers quiet. No DMM; calibration is FW-9.
- Firmware size, release build, `just fw-size`: FLASH 259.5 KiB of 510 KiB (0.13.0 recorded 243.5 KiB).

## [0.15.0] - 2026-10-15

### Added

- Plug and header power (API-11, FW-8, FW-14; `donguru_api` unchanged at 0.2.0): `plug.vbus_mode` and
  `header.vbus_sw_mode` (`ForceOff`, `ForceOn { max_voltage }`, `Auto { max_voltage, max_current, max_power }`, plug
  only) drive the switches through a firmware-side guard that checks the receptacle VBUS, the current and the power on
  every ADC update (~15 ms) and cuts the switch, reported as `Tripped(..)` in `plug.vbus` / `header.vbus_sw`; the
  AOZ1375 latch shows as `Fault`. A guarded mode above `max_voltage` is refused, a trip or fault stays until the next
  mode write. `plug.vbus_cycle(off, release_sbu)` (0..60 s) answers when VBUS is back on; `plug.data` cuts the DUT's
  USB data (U701), `header.relay` closes U101; all observable. Boot: everything off, data connected, relay open.
  Docs: `docs/user/power.md`. Tested 15 Oct 2026 on B153A + B129A at 5.2 V (on / off, refusals, OverCurrent /
  OverPower / OverVoltage trips, cycles of 100 ms to 2 s answered after the off time, pins through the J-Link); not
  tested: data cut in `lsusb` (needs a USB DUT), a U601 fault, a PD charger above 5.5 V. Needs wire_weaver SRV-20
  (deferred answers on nested levels).
- `donguru_power` crate: the modes and the guard as a host-tested state machine (API-11).

### Changed

- Firmware size, release build, `just fw-size`: FLASH 243.4 -> 260.7 KiB of 510 KiB, RAM 16.1 -> 16.6 KiB, `cnt` RAM
  60 -> 76 B (API-11: handlers and observe of the six properties, the deferred cycle answer, the guard and its ADC
  peaks).

### Removed

- The `plug-vbus-on` firmware feature: plug VBUS is off at boot and switched with `plug.vbus_mode` (API-11). A
  persisted boot mode (`device.settings.plug_vbus_at_boot`) comes with API-19.

## [0.14.0] - 2026-10-15

### Added

- donguru-core talks to the dongle over wire_weaver_client (CORE-1): enumeration from the USB descriptors (VID:PID
  `c0de:cafe` + API id `donguru_api`), selection by index / serial / USB path (auto when one is connected), the
  generated `donguru_api` 0.2 client and a typed facade per level of the API tree, the B153A net-name table for
  `debug`. Errors keep the device's category (`Refused`, `InUse`, ...) and typed error (`OverVoltage { measured }`).
- In-process mock dongle (`mock` feature, CORE-3): `DonguruApi` served on `ww_device::Server` in the same process,
  the test double for the CLI, GUI and Python bindings.
- `dg device list` and `dg device info` (CLI-7, text / json / jsonl). Tested 15 Oct 2026 on B153A on omarchy-m1:
  the dongle is listed (API 0.2.0), `device info` reads variant Main, board "Donguru B153" rev A, firmware, UID.

### Changed

- Exit codes: an item the firmware doesn't implement and an incompatible firmware API exit with 4 (device error),
  like a device error (CORE-2).

## [0.13.0] - 2026-10-15

### Added

- Device contract served (API-9, FW-11; `donguru_api` unchanged at 0.2.0, nothing changes for hosts):
  `device.firmware` (name, version, git SHA, build time, profile and the GNU build id, kept in flash, so `ww` finds
  the ELF), `device.last_reset` (reset cause, crash record, boot count, `clear`), `device.counters` (`cnt` RAM and
  backup words with the used counts and the build id, `reset`), `device.label` (name and notes, in RAM until the
  settings store, API-19), `leds` as `ww_indication` (`mode` `AllOff` / `OnlyEssential`, which keeps the power LED,
  / `Normal`; `brightness` in five steps; `identify` blinks both LEDs at full brightness for the given time, at most
  10 min, `Busy` while one runs). `device.firmware.BUILD_RECORD` is left out. Tested 15 Oct 2026 on B153A: `ww
  info`, `ww report`, `ww counters` complete (ELF by build id; log waits for API-10), label / LEDs from Python,
  identify seen in the LED timer registers.

### Changed

- A HardFault or panic no longer hangs the dongle: the record is saved and the MCU resets (FW-13); it shows in the
  boot log and in `device.last_reset`. With a debugger attached a panic still stops the core as panic-probe did.
  Tested 15 Oct 2026 on B153A with a forced HardFault (pc 0xFFFFFFF0): `ww report` shows `Fault`, pc and the
  symbolized stack; `last_reset.clear` empties the record. A panic is not tested on hardware.
- Both LEDs now run at one brightness, `Normal` (peak 128 of 255) by default: the power LED was at full brightness
  before. `leds.brightness` `Highest` gives the old look.
- embedded_bedrock template upgrade 0.5.0 (`f10b345`) -> `ec04ab4`: crash capture (`bedrock_crash`), GNU build id,
  `FirmwareInfo` / `LastReset` servers; `cnt` 0.5 (`fw/donguru/bedrock_fw.json`).
- Firmware size, release build, `just fw-size`: FLASH 228.6 -> 243.5 KiB of 510 KiB, RAM 14.4 -> 16.1 KiB of 144 KiB
  (API-9).

### Fixed

- `fw/donguru/examples/` build again (`usb_serial`: the CDC-ACM class moved to `cdc_acm::device` in the embassy
  fork; both examples take panic-probe as a dev-dependency).

## [0.12.0] - 2026-10-15

### Added

- `debug.led_set(led, mode)`: drive the power or status LED directly (API-8), behind the `debug-api` cargo feature
  (on by default).
- `device.*` contract constants: `firmware.FIRMWARE` (name, version, profile), `board.BOARD` ("Donguru B153" rev A,
  variant from the strap, serial), `uid.IDS` (the STM32 UID), `counters.INFO`, `log.DEFMT`, `leds.NAMES` (API-8).

### Changed

- `donguru_api` 0.2.0, the whole v1 API tree declared (API-8). **Breaks old hosts**: 0.1 and 0.2 don't connect (no
  host code used 0.1; `ww` and Python follow introspection). Moved: `swd_orientation` -> `plug.orientation` (now
  `Auto` / `Normal` / `Swapped`, observable; `Auto` acts as `Normal` until CC detection), `swd_mode` -> `swd.mode`
  (an unknown mode is refused with `InvalidArgument`), `swd_release` -> `sbu.release`, `board_variant` ->
  `device.variant`. Every new item answers `Unimplemented` until its piece lands. Built against wire_weaver
  `1a04005` (observe on both sides). Tested 15 Oct 2026 on B153A: `ww list` / `ww introspect` show the tree, the
  moved items read and write from Python, `plug.orientation` observed with a generated client (initial value and
  each change). Not tested: a CMSIS-DAP `probe-rs` read of B129A with `plug.orientation` (the DUT had no power;
  it is done with API-11).
- Plug VBUS is a mode (Roman, 15 Oct): `plug.vbus_mode` and `header.vbus_sw_mode` (rw + observe, `VbusMode`:
  `ForceOff`, `ForceOn { max_voltage }`, `Auto { max_voltage, max_current, max_power }`, open for more such as PD
  later) replace the sketched `vbus_on` / `vbus_off` / `VbusGuard`; `vbus` / `vbus_sw` keep the state with the trip
  reason (`GuardTrip` gains `OverPower`), `plug.vbus_cycle` returns to the mode. `device.settings.plug_vbus_at_boot`
  is a `VbusMode` (default `ForceOff`): it replaces the `plug-vbus-on` firmware feature once API-11 implements it.
  Declarations only, `Unimplemented` (API-8); not flashed (the B153A was lent to another session).
- USB endpoint registers are fixed: EP1 OUT / EP2 IN WireWeaver (double-buffered), EP3 CMSIS-DAP (API-8), so they
  don't move when an interface is added. Same order as before, so nothing changes for hosts.
- Firmware size, release build, `just fw-size`: FLASH 118.5 -> 228.7 KiB of 510 KiB, RAM 10.9 -> 14.4 KiB of 144
  KiB (API-8; the breakdown below was measured before the VBUS modes, at 227.3 KiB). Introspection with docs costs 8.5 KiB (no docs 217.8 KiB, none 212.8 KiB), under the 15 KiB rule, so
  the docs stay on the device. The `debug` level behind the `debug-api` feature saves 0.2 KiB when cut (its
  dispatch stays, the API is the same). At `opt-level = "s"` the same tree is 174.8 KiB (baseline 90.3 KiB).

### Removed

- `led_on` / `led_off` (API-2): `debug.led_set` and, with API-9, `leds` replace them.

## [0.11.0] - 2026-10-15

### Changed

- WireWeaver uses double-buffered USB bulk endpoints (embassy-rs/embassy#7275, from a git dependency until it is
  released): `ww usb-loopback` on a release build, 1 KB messages 214 -> 335 round trips/s, 419 -> 670 kB/s host ->
  device, 470 -> 520 kB/s device -> host; 64 B messages ~540 kB/s each way. Needs wire_weaver with
  `WireWeaverClass::with_endpoints` (FW-24). CMSIS-DAP enumerates and answers as before. Tested on B153A.

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
