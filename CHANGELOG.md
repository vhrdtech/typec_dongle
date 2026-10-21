# Changelog

All notable changes to Donguru: firmware, API crate, host library and CLI. The format follows
[Keep a Changelog](https://keepachangelog.com/), versions follow [Semantic Versioning](https://semver.org/). Feature
IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

### Changed

- Firmware: the shared clock follows one steerer at a time (bedrock_sync DEV-13, wire_weaver SRV-31): while one program's steer points keep coming, another one's are only noted, and it takes over after 3 s without the first; `clock.sync_status` names the followed (`steerer`) and the noted program (`noted`). No API change on Donguru's side (`ww_timebase` fields are `#[default = None]`). (FW-44)

## [0.20.23] - 2026-10-21

### Changed

- Firmware: the hub stays attached and configured through a firmware update, a crash or a reboot (FW-46), so a device on the plug keeps its USB link through a reboot or a crash; only Donguru's own USB device re-enumerates. At boot the firmware reads the hub's config back over I2C and keeps the hub when it holds what this firmware writes and is attached; a firmware with another hub config, or a hub that lost its config, gets the reset pulse and the config as before. A power cycle still starts the hub from reset. New counters `hub_kept` and `hub_kept_reconfigured`. No API change (`donguru_api` stays 0.5.1). Tested on B153A (omarchy-m1, 21 Oct 2026): the hub stayed the same host device through a probe flash, 3 `ww dfu reboot --into app` and a ROM update (`just update`); a firmware with a changed hub config reconfigured it, both ways; brown-out recovery (FW-18) still works; with a self-powered USB serial DUT on the plug (B129A) its open tty kept echoing through 5 reboots in a row, none lost. Not kept on rev A: the DUT's link through a ROM update, because the ROM bootloader drives the plug data switch off (PA9 is its USART1_TX, FW-50), and a bus-powered DUT, because plug VBUS goes off at every MCU reset (FW-49).

## [0.20.22] - 2026-10-21

### Changed

- Firmware: the LEDs show the common bedrock look (embedded_bedrock `bedrock_led::look`, DEV-15): power `OK` (the same
  3 s breath as before), status `HEARTBEAT` (the same 300 ms flash every 1.5 s as before), so Donguru looks like
  rePort and B135 for the same meaning. Visible change: brightness `Low` is the common peak 48 (was 40, no reason
  kept for it). The shared clock reports `acquiring` until it is within 2 ms of the host and slews 250 ms out in
  about 5 s instead of 25 s (bedrock_sync, DEV-13); `clock.sync_status` carries the slew left (`slew_us`) (FW-2,
  FW-44).

## [0.20.21] - 2026-10-21

### Added

- `donguru --version` / `dg --version` print the git commit (`-dirty` with uncommitted changes) and the build time:
  `donguru 0.20.20 (554cf91, built 21 Oct 2026 18:53 UTC)` (CLI-14). Scripts that parsed the old `donguru 0.20.19`
  line should match the first two words.
- Shell completion with live values for bash, zsh and fish (CLI-49): Tab after `--device` / `device select` lists the
  dongles plugged in (serial or USB path, label as help), `fw update` / `--elf` offer ELF files, `uart configure` offers
  baud rates and framings. `dg completions SHELL` now prints the dynamic registration script (same command, new
  script; `--bin dg` for the short name), `dg completions --install [SHELL]` writes `donguru` and `dg` into the
  per-user directories.
- `just install` (donguru, `dg` as a link to the same executable, completions) and `just deploy` (pull main, sibling
  checkouts, install, version check) (CLI-50).

### Changed

- `--color always|never` now also applies to `--help` and usage errors; `NO_COLOR` and `TERM=dumb` are honoured
  everywhere colour is chosen (the `fw update` bar and `swd memtest` output included).

### Fixed

- `dg power status | head -1` no longer prints `error: Broken pipe` and exits 0 when the reader closes the pipe (CLI-48).

## [0.20.20] - 2026-10-21

### Added

- Firmware: a boot loop ends in the STM32 ROM bootloader on its own (FW-45), so a firmware that crashes as it starts can still be updated over USB without a probe. Every boot counts in the boot word (TAMP_BKP4R); a boot is healthy once USB is configured or after 10 s up (a dongle on a charger never trips it). After 3 boots in a row without that, the fourth brings the hub up, resets into the ROM's USB DFU (0483:df11, the same serial) and `ww stm32dfu -s SERIAL flash ELF` writes a good image. The next firmware logs it and reports `device.dfu.node` `last_result` as `Recovery` until the next requested update or a power cycle (was always `Nothing`; no API change, `donguru_api` stays 0.5.1). Cargo feature `crash-loop-test` builds an image that panics at every start, to check it. Tested on B153A (omarchy-m1, 21 Oct 2026) without a probe: the crash-loop image written over USB ended in the ROM DFU behind the hub about 1 s after it started, `ww stm32dfu flash` brought the good image back with `Recovery` reported, the next `just update` cleared it, 5 app reboots in a row stayed in the firmware. Not run on a board: the rescue right after a power cycle, the 10 s mark without a USB host.

## [0.20.19] - 2026-10-21

### Added

- `dg fw version` shows the running firmware (name, version, git commit, profile, API version) and `dg fw update
  [ELF]` writes a firmware over USB without a probe: reboots into the STM32 ROM bootloader, writes and verifies,
  waits for the firmware and prints the old and the new version, with a progress bar on stderr and `--format json`
  (CLI-10). `donguru_core::update` has the typed API (`FirmwareImage`, `Dongle::update_firmware`,
  `Device::reboot`). Tested on B153A (omarchy-m1, 21 Oct 2026): three updates in a row, 44 s each.

## [0.20.18] - 2026-10-21

### Added

- Firmware update over USB without a probe, through the STM32 ROM bootloader (FW-12, API-58): `device.boot`
  (`ww_dfu::BootControl`) and `device.dfu.node` in `donguru_api` 0.5.1, a wire-compatible addition (older CLIs keep
  working with this firmware, this CLI with older firmware). `ww dfu -s SERIAL reboot` puts the dongle into the ROM's
  USB DFU behind its hub, `ww stm32dfu -s SERIAL flash ELF --wait 10 --follow` writes it and waits for the firmware
  to come back; `just update SERIAL` does both, `docs/user/firmware-update.md` explains it. Needs a `ww` from
  wire_weaver master (`ww stm32dfu`). On rev A the hub's RESET_N is driven from RTC_OUT1 so the hub stays up through
  the resets. Tested on B153A (omarchy-m1, 21 Oct 2026): ROM DFU up in under 0.5 s, 415 KB written and verified in
  42.5-42.9 s, firmware back right after, 5 times in a row without a power cycle (44 s per update end to end).

### Changed

- The firmware's USB serial number is now the 12-hex form the STM32 ROM bootloader reports for the chip
  (`204B319B5431` instead of `530016000A50315448313520` on the B153A), so `ww` follows the dongle into the ROM and
  back (FW-12). `--device` / `-d`, `ww list` and `ww -s` show and match the new serial; labels are unaffected. The
  full 96-bit UID stays in `device.uid` and `device.board`.
- Firmware: the backup-register counters (`bkp_cnt!`) get four of the five TAMP registers; the fifth is the boot
  word (FW-12).
- Firmware: the rev A workarounds are behind the cargo feature `rev_a`, on by default, so nothing changes for
  B153A (FW-47): PA4 driven high for the hub's SDA pull-up, the plug CC drive for SWD-host (FW-17) and
  `device.board` revision "A". Built without it the firmware follows rev B where its schematic is known (PA4 left
  alone, revision "B") and leaves the plug CC Hi-Z until rev B's debug-accessory Rp is decided. Both builds
  checked, not run on a board (no rev B yet).

## [0.20.17] - 2026-10-21

### Added

- Firmware: `clock` serves `ww_timebase::Clock` with host sync (`donguru_api` 0.5.0, API-57, FW-44): `now()` and
  `info()` from the embassy-time driver (TIM4, 1 µs resolution, no sync line); `steer`, `shared_now`,
  `sync_status`, `time_map`, `set_sources` / `sources` through embedded_bedrock's `bedrock_sync` (DEV-13), a static
  `SYNC` disciplined onto UTC by a host polling about once a second. `configure_sync` always `Unsupported` and
  `set_time_of_day` always `WrongState` (no sync line); no oscillator backend (Donguru's HSI16 trim is too coarse
  to steer without wobbling UART / PWM, decided 21 Oct 2026, P2528 design/clock-sync.md). The status LEDs
  (`src/led.rs`) align their breath / blink to `SYNC`'s shared time while it has one (`bedrock_led::Indication::
  run_with`), free-running otherwise as before, so Donguru's LEDs breathe in phase with other boards synced by the
  same host (`ww sync`). Release FLASH 391.59 → 407.04 KiB of 504 KiB (+15.45 KiB), RAM 89.11 → 89.27 KiB of
  144 KiB (+0.16 KiB). Flashed and run on B153A (omarchy-m1): boots clean, 227 s with no panic; `ww sync` end to
  end not yet exercised, blocked this session by the board's USB hub stuck from concurrent unrelated use of the
  same board (FW-44).

## [0.20.16] - 2026-10-21

### Changed

- Firmware and CLI built against WireWeaver master with TR-15 (link setup token, per-medium framing, 2bd3cc95): a wire break of the link, not of `donguru_api` (still 0.4.1). A dongle on older firmware no longer talks to this CLI and older CLIs don't talk to this firmware: flash the firmware and install the CLI together. Tested 21 Oct 2026 on B153A (omarchy-m1, flashed by J-Link): the new CLI against the 0.20.14 firmware stops with "The device speaks link 0.5.0 (without setup token) ... update the older side" (exit 1); after the flash `dg info`, `device list`, `device reset-cause`, `device counters --elf`, `power status`, `power meter`, `power watch` and `settings show` work, 30 reconnects in a row all connect, counters all 0.

## [0.20.15] - 2026-10-21

### Changed

- Firmware: the LED animation, its math and the task loop come from embedded_bedrock's `bedrock_led` (DEV-12, shared with rePort); `src/led.rs` only sets up TIM14 / TIM15, `ww/leds.rs` takes the brightness peaks (`Low` stays 40) and the identify busy / cap from `bedrock_led::indication` (FW-2, API-9). Same timings, defaults and API; the one change is the smoothstep, now rePort's exact one, still in 32-bit multiplies only: it rises monotonically (the old one stepped back by a count in 7932 places) and moves a compare value by at most 6 of 64000 (0.01 %), checked against the old loop over 2 million random ticks, otherwise identical. Release FLASH +32 B (400372 → 400404 B): the eight LED signals became one zero-initialised block in `.bss` behind one critical section per tick (was eight), `max_duty_cycle` is read once instead of every tick. LED task 192 → 248 B of RAM, its poll frame 112 → 128 B of stack (plus 16 B in `Leds::level` as before in `State::level`, 32 B in `Leds::set` on a mode change); the ease about 20 cycles per LED and tick (was about 16). Not run on a B153A yet.

## [0.20.14] - 2026-10-20

### Added

- Firmware: `uart[1].rx` is compressed with LZ4 for hosts that offer it (FW-42, WireWeaver API-55): 12 KiB of history, 4096-entry hash table, encoder run from RAM; hosts without the offer (CLIs before API-55) get the chunks as before. Tested 21 Oct 2026 on B153A at 5.818 Mbaud with the replay source: 482-568 kB/s of UART bytes delivered at 2.8-4.3x (top lossless at 568 kB/s), raw 505 kB/s; only with the embassy DMA ring fix (FW-42 open item), without it 16 kB/s.
- Firmware: `debug.uart_replay(capture)` (`donguru_api` 0.4.1, a patch bump, so old and new CLIs and firmware keep connecting: the new CLI checked against 0.20.13 firmware on B153A): with the cargo feature `uart-replay` the firmware loops a boot log, `ls --color` or `top` capture (32 KiB each, in flash) out of the header UART's TX by DMA, a full-rate source for the RX path through a jumper IO7-IO8; without the feature `Unsupported` (FW-42).
- Firmware: CPU idle counter `idle_us` and wake-up counter `executor_polls` in `device.counters` (FW-43): load = 1 - Δidle / Δtime.
- `donguru-core` example `uart_rx_bench`: replays each capture at a baud rate, receives `uart[1].rx` raw and with LZ4, checks every byte and prints UART kB/s, USB kB/s, ratio, CPU % and losses (`--closed`: the receive path alone).

### Changed

- Firmware: embassy from romixlab/embassy `donguru-ring-copy` (upstream 262fdba plus the DMA ring buffer copy fix: up to two runs instead of a software division per byte; the header UART got 16 kB/s at 5.8 Mbaud without it, FW-42). Back to embassy-rs once the fix is merged upstream.
- Firmware: the header UART's receive path reads the DMA ring (now 8 KiB) into 32 chunk slots instead of a 16 KiB pipe, and waits 1 ms after a read that didn't fill a chunk: on a busy line it no longer spins on 1-byte reads and the host gets full 512-byte chunks (FW-42). The server task sends at most 2 chunks per turn, so requests and the other tasks keep their turn when the line is faster than USB (before, `uart[1]` at 5.8 Mbaud made every request time out). FLASH 379.7 -> 386.4 KiB, RAM 63.2 -> 88.9 KiB (encoder 20.5 KiB, ring +4 KiB); `uart-replay` adds 96 KiB of flash.

### Fixed

- Builds against WireWeaver with `ww_board_info` 0.3 (STD-24): its optional `BoardInfo::REWORKS` is left out in the firmware and the mock (`omit = ["device.board.REWORKS"]`); hosts asking for it get `NotImplementedByDevice`.
- Firmware: bytes lost on `uart[1].rx` because the host was slower are reported at their position (before, a loss could be reported ahead of bytes received before it, shifting positions) (FW-42).

## [0.20.13] - 2026-10-20

### Added

- Firmware: `uart[1].configure` for the header UART (API-14): 4 Bd ..= 8 Mbaud, 7/8 data bits, N/E/O parity, 1/2 stop bits; answers the rate actually set (64 MHz / k: 6 Mbaud gives 5 818 182 Bd, -3.0 %), `Unsupported` for the rest. `capabilities` says 4 Bd ..= 8 Mbaud (was 921 600 only). Boots at 921 600 8N1 as before. RX DMA ring 1 -> 4 KiB and RX pipe 4 -> 16 KiB for the higher rates (`capabilities.rx_buffer` 16384). No `donguru_api` change. Tested 20 Oct 2026 on B153A with the IO7-IO8 jumper: 2 MB looped back exactly and in order, none lost, at 921 600 (927 536) Bd, 3M (3 047 619) and 6M (5 818 182): 70, 111 and 114 kB/s, the host -> device side caps it at ~120 kB/s (API-56); UART counters all 0.
- CLI: `dg uart configure BAUD [FRAMING] [--port header|plug]` (CLI-20) prints the line the dongle set and how far its rate is from the one asked for.
- `donguru-core`: `Uart::configure`; the mock answers it with the firmware's divider. Example `uart_resume --baud` sets the rate first and prints the rate back on rx.
- `just lint`: format check and clippy, what `just verify` runs before the tests.

## [0.20.12] - 2026-10-20

### Fixed

- Firmware: the hub watchdog (FW-18) no longer resets the hub when it doesn't answer on I2C (FW-41). The hub NAKs while it attaches and while the host enumerates it, which caused one or two needless hub resets 200-600 ms after every boot, and an endless reset loop every 0.6 s when the MCU's own USB was off the bus (seen after `probe-rs download` + `reset`, where probe-rs itself switches the USB clock off, see FW-41). The probe now reads the hub's VID/PID and recovers when they read all-zero twice (a hub reset by a brown-out waits for its config again); no probing for 1 s after an attach, and a USB-side request within 2 s of a recovery is taken as its echo, so one hub reset brings one recovery. Recovery logs say why (`hub: lost (ID registers read all-zero)`, `hub: lost (USB device left the configured state)`). Tested 20 Oct 2026 on B153A: no recovery after `cnt run` and three resets, one per forced hub reset (PC13 pulse through the J-Link), USB back each time.

## [0.20.11] - 2026-10-20

### Removed

- Firmware and mock: `gpio[].read_inputs` / `gpio[].write_outputs` (both answered `Unimplemented`), gone from `ww_gpio` (WireWeaver STD-30 dropped: one pin per call, many pins in one round trip as a multi request, same-instant multi-pin access through the logic analyzer / signal generator).

## [0.20.10] - 2026-10-20

### Added

- Firmware: the header UART, `uart[1]` (API-14, first part): USART5 on IO7 (header pin 1, TX) and IO8 (pin 2, RX), 8N1 at a fixed 921 600 Bd (`uart[].configure` is still `Unimplemented`). Its `tx` credit sink grants 2048 B (the TX pipe `src/uart.rs`), gets the credit back once the bytes are out on the line (the server task gives it back, it wakes for the UART), and its counted `rx` stream sends what the receiver's DMA ring collected as chunks of up to 512 B, bytes the firmware could not keep counted as lost in the next event; `capabilities` answers for it. `uart[0]` (plug SBU) is as before. Needs a jumper between header pins 1 and 2 to loop back. Hardware check: WireWeaver TEST-26.
- `donguru-core` example `uart_resume` (WireWeaver TEST-26): sends numbered lines on `uart[1].tx` while reading them back on `uart[1].rx` (jumper between header pins 1 and 2), drops the link mid-transfer (`--mode reconnect`: `disconnect().keep_streams()` and `Commander::reconnect()`; `--mode restart`: a second process continues from the saved cursor) and checks that every byte went out exactly once and in order; `--mock` runs it against the mock. The mock's `uart[1]` loops its tx sink back to its rx stream like the firmware with a jumper.

## [0.20.9] - 2026-10-20

### Changed

- Firmware: the `uart[].tx` credit sinks are on WireWeaver's sink protocol (API-34): their state is a `ww_stream::sink::SinkCredit` (was `ww_client_server::credit::SinkCredit`), opened by the host with a session it names, positions in bytes of the payload, a read on the sink returns its status. Needs a host with the same WireWeaver (the old sideband `Open`, `Credit`, `CreditQuery` and `Resend` are gone). Release FLASH at opt-level 3 362.6 -> 363.8 KiB (+1.2: the open, status read and `Query` dispatch), RAM +48 B (two `SinkCredit`s with sessions).

### Fixed

- Builds against WireWeaver with `ww_i2c` STD-35: its `#[optional(slave)]` group (I2C slave mode) is left out of the firmware's and the mock server's API (`omit = ["i2c.slave"]`), Donguru has no slave mode; a host calling it gets `NotImplementedByDevice` locally (or `Unimplemented` from the device). `just test` was red on main against WireWeaver master since STD-35 landed.

## [0.20.8] - 2026-10-19

### Changed

- Firmware: observed properties are event-driven (WireWeaver SRV-25): the server task sleeps until the next observed property is due (`ObserveState::next_wake`) or something changes outside it, instead of waking every 1 ms while a host observes anything (`OBSERVE_TICK` is gone). `src/power.rs` marks `plug.vbus` / `header.vbus_sw` changed through the observe signal (`ww::OBSERVE`, from any task; its `CHANGES` bits and `ServerState::power_changed` are gone), the ADC task checks the meter deadband on each new reading and marks `meter.reading` / `temperature` / `totals` there (the server task no longer polls `adc::latest()`), and the samples writer wakes the server for a batch. The log pump and the settings store's tidying keep their `PUMP_PERIOD`. Release FLASH at opt-level 3 355.7 -> 361.4 KiB (+5.8: `next_wake` and the signal, of which 3.2 come with the library), at z 243.7 -> 246.8 KiB (+3.1). Tested 19 Oct 2026 on B153A: the server task wakes ~101/s idle (the 10 ms log pump), ~111/s observing the meter on change or every 200 ms, ~190/s with samples (no 1 ms tick); `dg power watch` 50 rows in 5 s at 100 ms, 299 in 3 s at 10 ms; `--samples` 1608/s for 5 s, none missed (largest gap 759 us); with a 30 mV / 20 mA deadband an idle DUT sends the first reading only; on-change `plug.vbus` follows a `vbus_cycle` (Off, On 300 ms later, answer `On`) and an over-current trip (`Tripped(OverCurrent)` 8 ms after an `Auto` 5 mA mode). `ww usb-loopback` 1 KB: 311 round trips/s, 618 kB/s host -> device, 489 kB/s device -> host; 64 B: 2.0k/s, 302 / 514 kB/s, host -> device 17% below 0.20.6 (FW-40); with WireWeaver's `next_wake` early return and `is_active` flag 343 kB/s at 64 B (~5% below 0.20.6) and 621-628 kB/s at 1 KB, samples unchanged (1608/s, none missed).
- `MeterReading` has no `power` any more (`donguru_api` 0.4.0, P2534#19oct-1340): it was always `vbus_in` x `current`, now computed on the host (`donguru_core::reading_power_w`); `dg power` shows mW and `power_mw` in JSON as before. Saves a column of every batched meter sample on the wire and its encoder in the firmware. Update firmware and host together: hosts with `donguru_api` 0.3 can't observe or read `meter.reading` of a 0.4 firmware and the other way round (the API version tells them apart, `dg` says so). Release FLASH at opt-level 3 363.1 -> 355.7 KiB (-7.4), at z 246.4 -> 243.7 KiB (-2.8). Tested 19 Oct 2026 on B153A: `dg power status` / `watch` show mW as before.
- Firmware: handlers that never await are plain `fn`s, only the ones that do (settings and label writes, meter reads, `plug.vbus_cycle`, `sbu.release`, `adc.read`, `debug.log_flood`) stay `async` (WireWeaver SRV-21 `handler_model`); a property whose setter awaits keeps an async getter too. Release FLASH at opt-level 3 397.8 -> 385.2 KiB (-12.6), at z 282.2 -> 262.1 KiB (-20.2). Tested 19 Oct 2026 on B153A with SRV-22..25: settings show / set / reset, `plug.vbus_cycle`, the meter; device errors name their site (`BadIndex at level site 42 (index not valid)` for `uart[7]`, `Unimplemented at item site 2 (handler)`).
- Firmware: built with WireWeaver's fixed error site numbers (SRV-22): an error's `err_seq` names the same site in every build, and the error arms of the server are shared code. Release FLASH at opt-level 3 385.2 -> 383.5 KiB (-1.7), at z 262.1 -> 260.6 KiB (-1.5). No code change in the firmware.
- Firmware: built with WireWeaver's one observe table code (SRV-23) and one observe loop (SRV-24) for every trait level, instead of a copy per level. Release FLASH at opt-level 3 383.5 -> 363.1 KiB (-20.4), at z 260.6 -> 246.4 KiB (-14.2). No code change in the firmware.

### Fixed

- Observing `plug` sends its values (WireWeaver SRV-24 fix): `plug.cc` answers `Unimplemented`, and that one value made every `plug` update go out empty (`ww observe plug` showed `{}  missed 5`, also on 0.20.6), so `plug.vbus`, `vbus_mode`, `data` and `orientation` never reached a host. Now `cc` is left out and counted missed. Tested 19 Oct 2026 on B153A.
- `dg settings set meter_window` says the range the firmware takes, 0.9 ms..=14.9 ms (one ADC DMA half buffer, FW-3), instead of 1 ms..=1 s (15 ms and up were refused with `window out of range`), and sends the window in microseconds, so `900us` or `1500us` are no longer cut to whole milliseconds (CLI-47). Tested 19 Oct 2026 on B153A.

## [0.20.7] - 2026-10-19

### Changed

- Firmware: embassy from upstream main (embassy-rs/embassy 262fdba, pinned in `fw/donguru/Cargo.toml` `[patch.crates-io]`) instead of the romixlab/embassy `donguru-0.6-double-buffer` fork, now that the double-buffered bulk endpoints (embassy-rs/embassy#7275) and the G0 flash prefetch (#7276, turned off again here, FW-38) are merged; back to crates.io once the next embassy-stm32 release ships (FW-24). Ported to main's API: CamelCase PAC enums (`main.rs` clocks, `swd.rs` GPIO and SPI), `usb::Driver::new` takes the interrupt binding last, `uid::uid()` returns a reference, and the ADC on the unified driver (`adc.rs`): embassy now sets each channel's sample time (SMPSEL) and waits for CCRDY itself, so the SMPR rewrite and the CCRDY dance are gone; the 8-run averaged calibration stays (after `disable()`, as CCR and CALFACT need ADEN = 0). Checked on B153A, 19 Oct 2026: ADC1 SMPR reads 0x00300076 (SMP1 79.5, SMP2 160.5 cycles, SMPSEL12/13 = Vtemp/Vrefint on SMP2), meter samples 622 us apart as calculated for the Base set, 1 KB loopback 310-313 round trips/s, 642 / 492 kB/s, 64 B 2.1k/s, 368 / 520 kB/s (prefetch off on both, the fork build is within 2%); release FLASH 398.2 -> 401.8 KiB, RAM unchanged, stack high-water 10.0 KiB on both (painted, under `ww usb-loopback`).

### Fixed

- Firmware: the ADC waits 1 us after it is disabled before the calibration and the CALFACT write (the G0 asks for 2 ADC kernel clocks, else the ADC may freeze, efton.sk STM32 gotcha 221).

## [0.20.6] - 2026-10-19

### Fixed

- Firmware: flash prefetch is off again (FW-38). On the STM32G0B1 a prefetch can fail on a branch or call into the other flash bank (erratum ES0548 2.2.10, every silicon revision, no workaround but prefetch off), the CPU then runs wrong instructions; the firmware (~400 KiB) spans both 256 KiB banks, and a call from bank 2 into bank 1 HardFaulted under `ww usb-loopback`. Throughput on B153A, release opt-level 3: 1 KB 311-313 round trips/s, 631 kB/s host -> device, 491 kB/s device -> host (prefetch on: 332, 655, 512); 64 B 2.1-2.2k/s, 362-367 / 518 kB/s (2.3-2.4k, 441 / 536).

## [0.20.5] - 2026-10-19

### Changed

- Firmware: the panic and HardFault log lines are written with defmt directly (location and literal message; the registers of the exception frame) instead of through `core::fmt`; the crash record keeps the full panic text. Release build 1.4 KiB smaller at opt-level z, 1.7 KiB at 3. Not run on a board yet.

## [0.20.4] - 2026-10-19

### Added

- `dg settings show / set KEY VALUE / reset` (CLI-47, API-19): `show` prints every setting, `set` changes `plug_vbus_at_boot`, `button_action`, `meter_window`, `meter_deadband` or `uart_plug` (shell completion shows each `KEY`'s shape), `reset` puts everything and the label back to defaults (factory data stays) with no confirmation prompt, saying what it resets first. A `CommonError::Fault` from the settings store (flash write failed / a newer firmware's record / a record too big) now reads as a sentence instead of a raw code (`donguru_core::Error::fault_note`); the mock gained `settings_fault` to test it. No `--factory` flag: the design doc has one, the `Settings` API (API-19) doesn't.

## [0.20.3] - 2026-10-19

### Added

- Power meter samples (API-12, WireWeaver API-39): `meter.reading` observed in `Samples` mode sends every ADC sequence (~1.6 kHz, not averaged over `meter.window`) with its own device time, as compressed batches; `dg power watch --samples` prints them (text or jsonl). `donguru_api` 0.3.0: the `reading` observe slot changed, old CLIs can't observe the meter of a new firmware and the other way round (with the `ww_si` 0.2 change above, update both). Release FLASH 351 -> 397 KiB, RAM +3.8 KiB. Tested 19 Oct 2026 on B153A + B129A: 1608 samples/s for 5 s, none missed.
- Firmware: `device.settings` and `device.label` are kept in flash (API-19) by WireWeaver's `ww_settings` store in the last 8 KiB of bank 2, power-cut safe and read through the ECC-safe flash wrapper: label and notes, `plug_vbus_at_boot` (applied through the VBUS guard at boot), `meter_window` and `meter_deadband` (applied live and at boot), `button_action` and `uart_plug` (stored for FW-4 / API-14), `reset`. A write answers once it is in flash. `device.label` capabilities now say `persistent`. Code moved out of the last 8 KiB of flash (504 KiB for the firmware). Release FLASH 318 -> 351 KiB, RAM +2.3 KiB. The label at boot is also in the USB string descriptor, so `ww list` shows it. Tested 19 Oct 2026 on B153A + B129A: an existing store loaded, every setting written and read back after a reset, `plug_vbus_at_boot` switched plug VBUS on at boot through the guard; not tested: `reset`, a power cut mid-write, the `ww list` label.
- Lab book (PKG-5): `labs/`, 48 labs in six levels from first hour to insane, one Typst file each, built into one PDF (`just labs-book`) and web pages with a PDF per lab (`just build-labs`); each lab shows the status of the features it uses, read from FEATURES.md, and the book ends with a map of every feature the labs need.

### Changed

- Firmware, API crate, host library and CLI built against WireWeaver master of 19 Oct 2026 (`ww_si` 0.2, STD-38): quantities are one generic `Quantity<U>`, `adc` channel units are `SIExpr` (STD-37). The bytes of every value are the same, but the API model (and so the API hash) changed: a CLI and a firmware from before this change don't talk to each other, update both. Built, not yet run on a board.

## [0.20.2] - 2026-10-16

### Changed

- Docs: product names written WireWeaver and ShrinkWrap in prose (P2534).

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
