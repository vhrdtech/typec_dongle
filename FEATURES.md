# Donguru features and roadmap

This file is the single source of truth for what Donguru's firmware, host library and CLI do, what is broken and
what is planned, for humans and AI agents alike. [CHANGELOG.md](CHANGELOG.md) records what changed and when; the
[design docs](docs/design/index.md) say how things should work; this file records the current state.

Last full review: 3 Oct 2026 (commit `c702f0d`, 0.1.0). Built from the code, the design docs, `developer-notes.md`,
`bedrock_fw.json` and the B153A bring-up notes in tpm (P2534).

## How to use this file

- **Status** of each item:
  - ✅ done (for hardware: tested on a board, the note says which revision)
  - 🚧 in progress or partially done (the note says what is missing)
  - 🧪 implemented, builds, not tested on hardware yet
  - 🐛 implemented, but with known bugs
  - ⬜ stub: command, types or API exist but do nothing yet
  - 📋 planned
  - 💡 idea, not committed to
  - ⛔ blocked (the note says on what)
  - 🔍 probably done or obsolete, needs a check before closing
- **IDs** (`CLI-3`, `FW-5`) are stable: never renumber or reuse one. New items take the next free number of their
  area. Use the ID in commit messages, CHANGELOG entries and code `TODO`s (`// TODO(FW-8): ...`).
- **Target**: `revB` marks what testers need on the rev B dongles (end of 2026), `v1.0` what the CrowdSupply
  launch needs. No tag means not scheduled.
- Items are grouped by area. Each area lists what works first, then open items by priority.
- When finishing work, update the item in the same commit: mark it ✅ (or 🧪), add a pointer (module, command, API
  method) and move it up to the done items of its area. Don't delete done items. Items that turn out obsolete go to
  [Dropped and superseded](#dropped-and-superseded) with a one-line reason.
- A bug you find but don't fix gets an entry (🐛 on the feature, or a new item) with what triggers it.
- Small code-level gaps stay as `TODO` comments; only those that limit users or block a feature get an item.

## Firmware (`FW`)

- 🧪 **FW-1 Firmware base**: embassy on STM32G0B1CE, 64 MHz from HSI16 + PLL, USB clock from HSI48 trimmed by CRS,
  embedded_bedrock template 0.5.0 (`bedrock_fw.json`), defmt over RTT, `cnt` RAM and backup-register counters,
  fault handlers. 2K config page reserved at the end of flash (`memory.x`), not used yet. Stack overflow protection
  by flip-link (`linker` in `fw/donguru/.cargo/config.toml`; until 15 Oct 2026 the template's `build.rs` line had no
  effect): the stack sits below `.data` / `.bss` and an overflow faults. Not yet checked on a board.
- ✅ **FW-2 LEDs**: power and status LEDs on PF0 / PF1 with on / off / blink / breath animations and perceptual
  brightness (`src/led.rs`). B153A.
- ✅ **FW-3 ADC sampling**: continuous ADC1 into a DMA ring buffer with hardware oversampling and calibration;
  VDDA, die temperature, input voltage, input current, button / switch input, optionally P0 / P2 / P3
  (`src/adc.rs`, `adc::latest()`). B153A. Current sign: positive = charging the DUT (computer to DUT), negative =
  the DUT feeds the computer (0.1.1, from the schematic; check on the bench).
- ✅ **FW-4 Button and boot switch**: decoded from one ADC input (PA7) with debounce, edges and press duration
  (`src/button.rs`). B153A.
- ✅ **FW-5 USB hub bring-up**: hub works after I2C init (B153A, 29 Sep 2026; `examples/usb_serial.rs`). PF2 is
  NRST on this MCU, hub reset moved to PC13 for rev B.
- 🧪 **FW-6 Hub configuration**: bring-up moved from raw SMBus bytes to the `usb251xb` crate (`src/hub.rs`,
  `hub_config()`): same VID 0x0424 / PID 0x2512 (Main, USB2512B) / DID 0x0BB3 / config bytes as the FW-5
  bring-up, plus the port 2 and port 4 D+/D- polarity swap (`PortSwap`, `hw_b153a.md` §6) that the FW-5 bring-up
  never wrote, and a VID/PID readback logged right after configuring, using the crate's SMBus block-read framing
  (register address, then count + data) instead of the plain register read that used to come back as 0.
  Tested 14 Oct 2026 on B153A (J-Link via omarchy-m1, P2534#14oct-1402): HUB-1 (config/attach) and HUB-5
  (VID/PID readback, now `0424/2512` instead of 0) confirmed, `hw/b153_typec_serial/test/verify_b153a.md`.
  HUB-7 (the port swap itself, needs a device on the plug) not yet done — landing as is, HUB-7 later.
- 📋 **FW-7 USART on SBU with automatic TX/RX flip**: bridge to the host (CDC-ACM or over WireWeaver), flip by
  cable orientation. ^revB
- 🚧 **FW-8 Power switch and USB data switches** per downstream port, with safe defaults at boot: plug VBUS (U601),
  plug data (U701) and the relay through API-11 (`fw/donguru/src/power.rs`), all off / connected / open at boot.
  Tested 15 Oct 2026 on B153A (API-11). Open: the Plus variant's hub port 3 / 4 (no per-port hub control in v1). ^revB
- 📋 **FW-9 Power meter calibration**: offsets and gain per board, stored in the config page. ^v1.0
- 📋 **FW-10 GPIO and I2C** on the external pins. ^v1.0
- ✅ **FW-11 `fw_info` / `hw_info`** (embedded_bedrock device contract): served as `device.firmware`
  (`ww_firmware_info::FirmwareInfo`: name, version, git SHA, build time, profile, GNU build id kept in flash by
  `memory.x`) and `device.board` (`ww_board_info`: "Donguru B153" rev A assumed, variant from the PA10 strap, UID as
  serial), `fw/donguru/src/ww/device.rs` (API-9). Template upgrade to embedded_bedrock `ec04ab4`. Tested 15 Oct 2026 on B153A
  (`ww info`). ^revB
- 📋 **FW-12 Firmware update without a probe**: STM32 system bootloader via the boot switch first, then a custom
  bootloader; the CLI side is CLI-10. ^v1.0
- ✅ **FW-13 HardFault handling for production**: `HardFault` and the panic handler save a `bedrock_crash` record
  (cause, registers, 32 stack words, message, uptime, build id) in `.uninit` RAM and reset instead of looping
  (`fw/donguru/src/main.rs`, `src/last_reset.rs`); the next boot logs it and serves it as `device.last_reset`
  (API-9). The `hard_faults` backup-register counter still counts. Tested 15 Oct 2026 on B153A with a forced fault
  (PC set to 0xFFFFFFF0 through gdb): reset, `Fault` with pc and a symbolized stack in `ww report`. A panic is not
  tested on hardware; `exc_return` reads 0 on the M0+ (bedrock_crash's best effort, the frame is still right).
- ✅ **FW-15 SWD-host over the plug SBU lines**: CMSIS-DAP v2 (bulk, `src/cmsis_dap.rs`) on a composite USB device
  alongside WireWeaver (`src/ww/`), carrying `dap_rs`'s command processor over a GPIO bit-bang SWD driver
  (`src/swd.rs`: SWCLK/SWDIO on PA1/PA0, the SBU side of the TXS0102 level shifter, orientation-swappable, SWCLK
  clamped 10 kHz..4 MHz since FW-20, default 400 kHz). R211/R212 (the shifter bypass to PB8/PB6) are not fitted on B153A, so
  the driver goes around the shifter: `SBU_OE` (PB2) and `SBU_PU_EN` (PC15) always low, V_SBU at 3.3 V (PD3 high),
  so 3.3 V targets only. No hardware reset line (NRESET/NTRST): `SWJ_Pins` reports it as never asserted. JTAG and
  SWO not wired (SWD only). Orientation is a runtime setting (`plug.orientation`, API-4), not
  auto-detected from CC polarity. The SWD-host pins go Hi-Z on a clean `DAP_Disconnect` and also when the CMSIS-DAP
  v2 endpoints get disabled (USB reset/suspend, probe-rs or its process going away without disconnecting
  cleanly), not just on an explicit `sbu.release` (`src/cmsis_dap.rs::dap_task`): a bench harness only
  has to end the probe-rs session, one way or another, before cutting the DUT's power.
  Tested 12 Oct 2026 on B153A + B129A (CANnify Micro, STM32G0B1) with probe-rs: reads at 100 kHz, 400 kHz and
  1 MHz (16 KB in about 2 s), both cable orientations (flipped = `Swapped`), pins Hi-Z after disconnect.
  Next: a level-shifter mode for 1.8 V targets (P2534#12oct-1338b); SPI-driven SWD is FW-21.
- ✅ **FW-17 Plug CC drive for SWD-host**: stopgap, hardcoded (`src/swd.rs`, `src/cmsis_dap.rs`; P2534#12oct-1338b).
  PD0/PD2 (UCPD2_CC1/CC2; B153A has no DPO2036, hw_b153a.md §0, so these reach the plug CC directly) are driven
  push-pull high on a successful `DAP_Connect` and released to Hi-Z together with SWCLK/SWDIO on `DAP_Disconnect`,
  CMSIS-DAP v2 endpoint disable and `sbu.release` — a downstream board such as B129A gates its SBU->SWD mux on CC
  voltage and would otherwise never route its SBU lines to the MCU, and a driven CC left up after a session would
  back-feed an unpowered DUT through the gating comparators. Not a real CC/PD role: no Rp/Rd, no UCPD2 involved;
  the follow-up (proper CC/PD handling) is tracked on the tpm task. Tested 12 Oct 2026: B129A's `SWD_SEL` (TP1)
  goes high during a session, PD0/PD2 are inputs again after disconnect.
- ✅ **FW-18 Hub brown-out recovery**: the hub (U301) shares `+3V3_DCDC` with the MCU (hw_b153a.md §3); a hard
  turn-on load on the header's switched VBUS (Q102, inrush into a large downstream cap) can sag that rail enough
  to reset the hub without resetting the MCU, which loses the hub's SMBus config and attach state for good
  (P2534#12oct-1819, seen on the P2625 power-cut bench). `src/hub.rs`'s `watchdog_task` notices the loss — the
  composite USB device (carried on hub port 1) dropping out of the configured state it had reached, or a periodic
  I2C presence probe — and redoes the reset + config + attach bring-up.
  Tested 12 Oct 2026 on B153A (P2625 bench, omarchy-m1): the bench's VBUS_SW line now has a 10R series resistor
  (power-cut-bench session) that stops the original inrush from browning out the hub, so this was verified by a
  debugger-forced hub reset instead (same electrical event as the brown-out, from the hub's point of view): a
  J-Link (`--probe 1366:0105`) wrote PC13 (GPIOC BSRR, `0x5000_0818`) low then high while the firmware kept
  running, 10 times. Donguru's composite device (VID:PID `c0de:cafe`) re-enumerated on the host every time, about
  2.0-2.3 s after each pulse (`journalctl -k`: the hub itself, VID:PID `0424:2512`, re-attaches first). The ~1 s
  target in the tpm task was a rough estimate; ~2 s here is mostly host-side USB re-enumeration overhead, not the
  firmware's own recovery time. Not yet exercised against a real brown-out (needs the 10R resistor bypassed, which
  the user declined for now since the bench is in use for cut measurements).
  🐛 Known quirk (B153A, 14 Oct 2026, FW-6 hardware check): `watchdog_task`'s periodic I2C probe fires one or two
  spurious recoveries about 200-400 ms after a cold boot, before the hub has finished its own internal attach and
  well before USB enumeration — harmless (self-heals before the host notices) but not a real brown-out. Confirmed
  present on the unmodified 0.6.0 firmware too, so not from the FW-6 crate change. The obvious fix is a short
  grace period after [`bring_up`](../fw/donguru/src/hub.rs) before `watchdog_task` starts probing; not done in
  this branch.
- ✅ **FW-19 512-byte CMSIS-DAP packets**: `DAP_Info` reports `MaxPacketSize` 512 (`dap_rs` feature `usb-hs`, which
  changes nothing else) instead of 64, so `DAP_TransferBlock` carries 126 words per USB round trip instead of 15.
  The endpoints stay Full-Speed 64-byte bulk: `src/cmsis_dap.rs` puts a request together from several OUT packets
  and splits a response over several IN packets. The rules are in the `donguru_dap` crate (repo root, unit-tested
  on the host):
  - A request ends where its own bytes say (command lengths parsed per the CMSIS-DAP layouts), not at a short
    packet or after a pause: probe-rs 0.32 sends no zero-length packet after a request that is an exact multiple
    of 64 bytes (25 batched register writes are 128), and a pause cannot tell a slow host from a finished request.
  - A request whose rest does not arrive within 100 ms is dropped and the next one starts clean; one cut short
    by a short packet, one longer than 512 bytes or one asking for a response over 512 bytes is dropped too (no
    response, counters `dap_requests_truncated`, `dap_requests_too_long`, `dap_responses_too_long`). This also
    closes a panic on host input: `dap_rs` indexes past the end of a truncated request.
  - Commands with a layout the firmware does not know (vendor, UART, `DAP_ExecuteCommands`) end at a short packet
    or after 10 ms of silence.
  - A response that is an exact multiple of 64 bytes and shorter than 512 gets a zero-length packet (probe-rs
    reads more than the response holds); the host gets 1 s to read a response (`dap_response_timeouts`), then the
    task goes back to listening, so `sbu.release` is never stuck behind a host that stopped reading.

  Hi-Z on `DAP_Disconnect`, on endpoint disable and on `sbu.release` is unchanged.
  Tested 13 Oct 2026 on B153A + B129A (omarchy-m1, probe-rs 0.32, release build with `plug-vbus-on`, before =
  0.4.1): the link works and is clean, but the gain is about 9 %, not the 4-5x expected. 64 KB of B129 flash at
  1000 kHz: 7.7 s -> 7.05 s with 32-bit accesses (`probe-rs read b32`), 30.3 s -> 27.3 s with 8-bit, bytes
  identical to the read before. `donguru swd memtest --size 16K` at 100, 400 and 1000 kHz and the full 144 KiB RAM
  at 1000 kHz: all patterns pass (2.1 / 6.0 / 9.6 KiB/s; 9.0 KiB/s at 1000 kHz before). PA0/PA1 and PD0/PD2 are
  inputs without pull after the session (GPIOA/GPIOD MODER and PUPDR read through the J-Link). The `dap_*`
  counters were not read.
  Why so little: the USB round trips were not the bottleneck, the bit-bang is. 16384 words in 7.05 s is about
  430 us per word where a real 1 MHz SWCLK needs about 50 us, and the rate still follows the requested clock
  (2.1 KiB/s at 100 kHz): `src/swd.rs` spends several microseconds per bit on top of the half-period delay. The
  next step for speed is there (a tighter bit loop, or SPI-driven SWD as planned under FW-15), and one request
  now holds the executor for about 55 ms at 1000 kHz (126 words) and about 0.25 s at 100 kHz.
- ✅ **FW-20 Faster bit-bang SWD**: the SWD clock is what was asked for, up to about 3 MHz (P2534#13oct-1353).
  FW-19 showed the bit loop was the limit: about 50 CPU cycles of overhead per clock edge (embassy `Flex` calls,
  the orientation re-read per edge), so "1 MHz" ran at about 400 kHz. `src/swd.rs` now clocks through two
  inline-assembly kernels on GPIOA's BSRR / BRR / IDR (one store per edge, SWCLK falling and the next SWDIO bit in
  the same write), with the orientation resolved once per transfer and the SWDIO direction switched through MODER
  (pull-up kept). Each clock half is a countdown loop; how many iterations give the requested clock comes from a
  calibration at boot that times the same kernels with SysTick while the pins are still inputs (the plug sees
  nothing), arithmetic in `donguru_dap::swclk` (unit-tested). The clock never runs faster than asked
  (`DAP_SWJ_Clock` is a maximum); anything from 3 MHz up gives the kernels' fastest, about 3 MHz real. The boot
  log prints the measured costs and the top SWCLK. Both clock halves are the same length at every speed: each
  kernel does the next bit's work in the low half and its loop counter in the high one, and an odd spin goes to
  the low half. Cap 3 MHz (`MAX_FREQUENCY_HZ`), default still 400 kHz; sampling before the rising edge, SWCLK
  driven on every path and the Hi-Z release unchanged.
  The usable clock also depends on the DUT: series resistors and clamp diodes on its SWD lines slow the edges
  (B129A: 200R + 5V6 zener, tau about 70 ns), so a short clock half no longer reaches a clean low. On B129A use
  2 MHz or less.
  Tested 13 Oct 2026 on B153A + B129A (omarchy-m1, probe-rs 0.32, release build with `plug-vbus-on`, flashed and run
  by the parent session): 64 KB of B129 flash with `probe-rs read b32`, bytes identical to the 0.5.0 read, in
  9.96 / 3.13 / 1.77 / 1.27 / 1.07 s at 100 / 400 / 1000 / 2000 / 4000 kHz (0.5.0 at 1000 kHz: 7.03 s; 4000 kHz is
  about 60 KB/s). `donguru swd memtest --size 16K` at 400, 1000, 2000 and 4000 kHz and the full 144 KiB RAM at
  4000 kHz pass (72 s, 493 s at 1000 kHz on 0.5.0); 100 kHz memtest not run. PA0/PA1 and PD0/PD2 inputs without
  pull after the session. The real SWCLK was not measured (no logic analyzer channel on SWCLK); derived from the
  read times (time per word = fixed overhead + 54 clocks / f, about 50 us of overhead), it is within a few percent
  under the requested 100, 400 and 1000 kHz. Scope (M2K at 100 MS/s on B129 J2-9 SWCLK / J2-10 SWDIO, flying
  leads, by the parent session): 1000 kHz asked -> 0.97 MHz, 53 % duty, clean 0..3.18 V; 4000 kHz asked -> median
  period 330 ns (3.03 MHz, fastest 240 ns) at 70 % duty (high about 230 ns, low only about 100 ns), the clock at
  B129 swinging only about 0.7..3.1 V (the DUT's RC above; G0 VIL is 1.0 V), memtest still passing. The equal
  halves above are the fix for that. Re-measured with the fix (babf602, same setup): 1000 kHz -> 0.98 MHz at 49 %
  duty, -0.07..3.18 V; 2000 kHz -> 2.00 MHz at 52 %, 0.06..3.09 V; 3000 kHz -> 2.78 MHz (360 ns) at 47 %, low
  0.16 V (worst 0.28), high 2.89 V (worst 2.82). 64 KB reads identical in 1.77 / 1.29 / 1.19 s at 1000 / 2000 /
  3000 kHz, memtest 16K at 400 / 1000 / 2000 / 3000 kHz pass, Hi-Z after disconnect holds.
- 🧪 **FW-21 SPI-driven SWD** (experimental): a second SWD-host mode, `SwdMode::Spi` (API-5), next to the default bit-bang
  (P2534#13oct-1353b). On B153A the SPI pins reach the plug only through the TXS0102 level shifter (R211/R212 not
  fitted, hw_b153a.md §8), so the mode engages that side: PA0/PA1 Hi-Z, SPI2 on with SWCLK = SPI2_SCK on PB8 and
  SWDIO = SPI2_MOSI on PB11 + SPI2_MISO on PB6 (same net, full duplex, MOSI switched to input while the target
  drives), then `SBU_OE` high; V_SBU stays 3.3 V. Every frame CPOL 0 / CPHA 0, LSB first, 4..16 bits: request 8 |
  turnaround + ack 4 | read: data 16 + 14 + (2 data + parity + turnaround) 4, idle 8 | write: (turnaround + 15
  data) 16 + 16 + (1 data + parity + idle 8) 10. MOSI only drives where the target is done or drives the same level
  (the OK ack's last bit is 0); WAIT / FAULT get a bit-banged turnaround and idle on the same pins, and so do SWJ
  sequences (any bit count) and clocks under 250 kHz (`src/swd.rs`, "SPI-driven SWD"). SPI clock = 64 MHz / 2^n,
  never above the request, cap 4 MHz (`SPI_MAX_HZ`). Normal orientation only: SBU1_VT has no MISO
  pin, a swapped cable falls back to the direct bit-bang. Every release (`DAP_Disconnect`, endpoint disable,
  `sbu.release`) takes `SBU_OE` low first (its 10K pull-ups on both sides go with it), then PB6/PB8/PB11 Hi-Z and
  SPI2 off; switching back to bit-bang does the same. Selected at run time with `swd.mode`, or at boot with the
  firmware feature `swd-spi` (the CLI has no command for it yet, CLI-17). ⚠️ Not for a board with R211/R212 fitted.
  Tested 13 Oct 2026 on B153A + B129A (release build with `plug-vbus-on,swd-spi`, by the parent session): 64 KB
  reads identical at 100 / 1000 / 2000 / 4000 kHz in 10.02 / 1.86 / 1.40 / 1.24 s, 8000 kHz fails (protocol
  error); memtest 16K at 100 / 1000 / 2000 / 4000 kHz pass; after disconnect PB2 is low and PB6/PB8/PB11, PA0/PA1,
  PD0/PD2 are inputs. But the signal through the TXS0102 is poor at B129A: SWCLK highs peak at only about 2.3-2.5 V
  at 1 MHz and about 2.0 V at 4 MHz (lows reach 0 V), host-driven SWDIO highs ramp toward 3 V over microseconds.
  After its one-shot the TXS0102 holds a high only through its 10K pull-up, against B129A's 10K pull-down (R28) on
  SWCLK (a divider at about 1.6 V) and the 200R + zener capacitance: the G0's Schmitt inputs cope, but it is below
  VIH (0.7 VDD) on paper. Not for targets with pull-downs below about 50K on SWD. And it is not faster than the
  bit-bang here (1.24 s at 4 MHz vs 1.19 s for FW-20 at 3 MHz), so bit-bang stays the default and SPI stays
  experimental: 🧪 on purpose, no more bench time planned.
- 🧪 **FW-22 Board variant detection**: Main (2-port USB2512B) vs. Plus (4-port USB2514B, not decided/built yet)
  identified at boot from a GPIO strap, not the hub's PID (`src/hub.rs::read_board_variant_strap`,
  `donguru_hub` crate, P2534#14oct-1439). Reading the hub's own power-on-default VID/PID/DID (HUB-5) before
  configuring it does not work on this hub's SMBus-slave strap — it reads all-zero regardless of delay (tried
  0/5/100/1000 ms), confirming hw_b153a.md §9's existing caution that these registers hold whatever was last
  written there, not a factory default. The signal used instead: PA10 (`SBU_TO_USB`, U901 S) carries R902, a
  10K pull-down fitted only on Plus (hw_b153a.md §9/§11); read against the MCU's own ~40k pull-up after
  releasing PA10's UCPD1 dead-battery pull-down (`SYSCFG.cfgr1().ucpd1_strobe`, resolves the 🔍 open in
  hw_b153a.md §11), high = Main, low = Plus. `donguru_hub::hub_config` then builds the hub's config
  (`usb251xb::Variant`, port count, D+/D- swap mask for FW-6) from the detected variant instead of a hardcoded
  Main constant; the pure mapping and config builder are host-unit-tested for both variants (`donguru_hub`,
  `just test`), including the not-yet-built Plus. Variant exposed read-only over WireWeaver
  (`device.variant`, API-7, FW-11 hw_info). Tested 14 Oct 2026 on B153A (J-Link via omarchy-m1,
  P2534#14oct-1439): reads PA10 high → `BoardVariant::Main` on 2 cold power-cycles (true replug, J-Link VTref
  0.00 V between them) and 2 debugger-forced resets, hub attaching correctly every time. 🧪 because Plus is
  untested hardware.
- 🐛 **FW-23 Firmware not clippy clean**: `cargo clippy --bins --examples -- -D warnings` in `fw/donguru` fails with
  13 lints (doc list indentation in `hub.rs` / `swd.rs`, operator precedence in `led.rs`, an empty `loop {}` in
  `main.rs`, unused `ChannelSet::Extended`, `adc::set_channels` and `Brightness::Dim`); `just clippy` covers only
  the host workspace, so nothing catches them. Found 15 Oct 2026.
- 🚧 **FW-24 WireWeaver USB throughput** (P2534#15oct-1747): was ~30% of USB FS bulk (`ww usb-loopback`, 1 KB
  messages: 152 round trips/s, 373 kB/s host -> device, 316 kB/s device -> host). Done: flash prefetch on
  (`main.rs`, embassy-stm32 0.6 leaves it off on the G0); release `opt-level = 3` instead of "z" (~40% faster
  here); WireWeaver TR-12 (U2Head byte fast path, replies flushed when no request waits); double-buffered bulk
  endpoints (`src/ww/mod.rs`, `WireWeaverClass::with_endpoints`) from embassy-rs/embassy#7275, through a git dependency on
  the romixlab/embassy `donguru-0.6-double-buffer` branch (`fw/donguru/Cargo.toml` `[patch.crates-io]`). Release
  build now: 1 KB 335 round trips/s, 664-676 kB/s host -> device, 520 kB/s device -> host; 64 B 2400-2500 round
  trips/s, 540 kB/s each way. Tested 15 Oct 2026 on B153A. Open: back to crates.io embassy once #7275 is released;
  device -> host stays ~520 kB/s with the device ~30% idle (CRC burst per message, copies: WireWeaver TR-12).

- ✅ **FW-25 B129A answers no SWD through the CMSIS-DAP** (not a Donguru bug): from 15 Oct 2026 `probe-rs read
  --probe c0de:cafe` got a timeout / a protocol error from B129A, also with v0.6.0, the firmware that read it on
  13 Oct. Cause, as far as seen from here: B129A's core was held in reset by the unpowered M2K (ADALM2000) whose scope leads were still on its
  J2 from the 13 Oct measurements (16 Oct 2026, fw25). The SWD link itself worked all along (DPIDR 0x0BC11477,
  debug power-up acknowledged), but DHCSR read 0x02000001 with S_RESET_ST set again on every read, no halt even with
  VC_CORERESET + C_HALT (pyOCD attach), RAM / RCC / DBGMCU read 0, and a plug VBUS power cycle didn't change it.
  With the M2K powered again (USB, 16 Oct 11:54) the core runs (DHCSR 0x01000001, DBGMCU IDCODE 0x10016467) and
  B129A draws 18 mA at 5.23 V instead of about 10 mA. Checked on the way: the SWD wire path in `swd.rs` /
  `cmsis_dap.rs` is unchanged since v0.6.0 apart from formatting; during `DAP_Connect` the B153A drives PA0/PA1,
  PD0/PD2 high, PD3 high, PA6 on, PC14 no fault (J-Link reads); with CC high SBU2 (PA0) is pulled up (target
  SWDIO) and SBU1 (PA1) held low by R28 (SWCLK), so B129A's SWD mux switches. Lesson for a bench DUT: an unpowered
  instrument left on its pins can hold it in reset; an attach that reads DPIDR but never halts points there.
- 🐛 **FW-26 ADC DMA overrun while writing to a host that died**: a host process killed without disconnecting leaves
  the server task writing (log stream events since API-10, observed properties before) until the 250 ms packet
  timeouts end in "USB write timed out"; one or two `ADC: DMA ring buffer overrun` (`adc_overruns`) show up in that
  window, seconds after the kill, so the meter loses a window. Seen 16 Oct 2026 on B153A with the API-10 firmware;
  no overrun idle for 2 min nor with a host reading the log for 60 s. Not checked on main's firmware with an observer
  killed the same way. Suspect: USB interrupts while the IN endpoint NAKs (double-buffered endpoints, FW-24). Seen
  again with the non-blocking RTT tee (FW-27) and no probe attached, ~2 s after the timeout: not the RTT.
- ✅ **FW-27 RTT log never blocks**: the firmware runs the same with or without a probe: `ww_defmt_logger`'s RTT tee
  writes each frame whole or skips it, with the channel forced to NoBlockSkip before every write, as probe-rs
  switches it to blocking on attach. Code: WireWeaver `ww_defmt_logger` (`tee_frame`, `rtt_skipped()`, host test
  `rtt_tee_never_blocks`), `fw/donguru/src/log.rs`; `debug.log_flood` yields between lines. Tested 16 Oct 2026 on
  B153A: three floods of 1000 with probe-rs attached and three without, each 227 lines + "773 lost" over USB, no ADC
  overrun from any of them (probe-rs showed what fit, 753 of 3000 lines); a line costs ~62 us instead of ~620 us
  with the blocking channel. Before, the flood overran the ADC twice over: the blocking RTT channel and a flood
  handler that never yielded (~62 ms in one go, still an overrun with the tee fixed).
- 💡 **FW-28 Standalone mode**: what Donguru does when it powers up with no computer on the receptacle (a charger, a
  power bank): `device.settings.standalone` = `Off` (default) / `PdInspect` (ask the source for its capabilities,
  blink the result, keep it for `dg pd last`) / `PdBridge { max_voltage }` (sink to the charger, source to the DUT,
  both conversations kept, API-30) / `RunSeq` (FW-33) / `PowerLog` (record the meter into flash until a computer
  reads it).
  The plug stays off unless the mode itself switches it, within its limits. Labs `charger-inspector`,
  `pd-both-sides`, `voltage-sweep`. Needs API-23, API-19.
- 💡 **FW-29 Meter history on the device**: one averaged point a second (V, I, P, energy) for the last 24 h in a RAM
  ring (~17 KiB), so a host that sleeps or reconnects backfills its log without holes; `meter.history(since)` as a
  stream. Lost on a power cut of the dongle itself. Lab `overnight-log`.
- 💡 **FW-30 Fast current trip from the ADC analog watchdog**: the G0's ADC AWD on the current channel opens the plug
  switch from its interrupt within microseconds of a sample beyond the trip, instead of the guard's ~15 ms update;
  the guard keeps the slower voltage and power checks. Needs the trip current in ADC counts after FW-9 calibration.
  Lab `safe-first-power-on`.
- 💡 **FW-31 Watchdog on the dongle**: the DUT shows it is alive by toggling a pin (header IO or plug SBU) or printing
  a line on the SBU UART; when it stays silent for `timeout` (after a `grace` for booting) Donguru cycles the plug
  VBUS by itself, counts the cycles (persistently) and keeps going with no computer. Lab `hung-board`.
- 💡 **FW-32 Precisely timed power events**: `plug.vbus_cycle` gains `after` (delay from the call, so a host-side
  trigger lands at an exact point) and a `pulse(off)` for dropouts of ~100 µs to ms, driven from a timer, not the
  executor. How short a dropout the DUT really sees depends on the AOZ1375's turn-on: measure it. Lab
  `power-cut-test`.
- 💡 **FW-33 Sequences on the dongle**: a short list of steps stored in flash (PD request / PPS level, power on/off
  with limits, sweep, wait, mark) run by the firmware without a computer (FW-28 `run-seq`), with the meter logged
  per step for reading back. Small interpreter, no scripting language. Lab `voltage-sweep`.
- 💡 **FW-34 RTT reader on the dongle**: Donguru finds the DUT's RTT control block over its own SWD link and streams the
  up-channels to the host, stamped with the meter's clock, so log and current share one time axis without
  probe-rs. defmt decoding stays on the host (ELF). Excludes a probe-rs session at the same time. Lab
  `logs-and-current`.
- 💡 **FW-35 Plug-side VBUS sense**: the voltage after the switch and shunt (`+V2`) on an ADC channel, as
  `meter.reading.vbus_out`: the DUT's real voltage, the switch drop, back-feed seen with the plug off. Rev A: a board
  mod (two resistors to a free ADC pin) behind the feature `plug-vsense`; rev B: the divider on the board. Labs
  `plug-voltage-mod`, `daisy-chain`.
- 💡 **FW-36 FDCAN on the plug**: FDCAN1 on SBU (PB8 / PB9) and FDCAN2 on A2 / B2 (PB13 / PB5), classic CAN and CAN
  FD, logic level (a transceiver on the DUT side for a real bus); A2/B2 have no over-voltage protection. Lab
  `can-on-the-plug`.
- 💡 **FW-37 Logic analyzer on the header**: timer-paced DMA sampling of IO7..IO10 into a RAM buffer with a trigger and
  pre-trigger, streamed to the host; a few MS/s buffered, less continuous (USB FS). Lab `logic-analyzer`.

## API (`API`)

- 🧪 **FW-14 Switched VBUS on the header** (J101 pin 10, PD1 `5V_EN`, net to be renamed VBUS_SW): it is the computer's
  VBUS as it is, 5 V or up to 20 V after PD. Off at boot; `header.vbus_sw_mode` `ForceOn { max_voltage }` cuts it
  when the receptacle VBUS goes above the limit (API-11); `Auto` is refused, its current isn't measured (the shunt is
  on the plug path only). Tested 15 Oct 2026 on B153A at 5.2 V (cut on a later rise above a lowered limit, PD1 seen
  high); not with a PD charger above 5.5 V, so 🧪. The receptacle voltage with the state is the meter's (API-12). ^revB
- ✅ **API-1 WireWeaver over USB**: `DonguruApi` served by `fw/donguru/src/ww/` (USB FS, VID:PID `c0de:cafe`, API
  id in a string descriptor for `ww list`). Fixed endpoint registers: EP1 OUT / EP2 IN WireWeaver (double-buffered),
  EP3 CMSIS-DAP (API-8). Tested 15 Oct 2026 on B153A with `ww list` / `ww introspect`.
- 📋 **API-3 Full API**: ports (power, data), power meter readings and streams, button / switch events, USART bridge,
  GPIO, I2C, device info; designed together with the CLI command tree. ^revB
  Planned 15 Oct 2026 (P2534#15oct-2031): `donguru_api` 0.2.0 with the whole tree declared, then one item per level
  below (API-8..API-21); the design is kept in tpm (P2534 `design/api-v1.md`).
- 🧪 **API-4 `plug.orientation` / `sbu.release`** (were `swd_orientation` / `swd_release`, moved in 0.2.0, API-8):
  cable orientation of everything on SBU (rw + observe, `Auto` / `Normal` / `Swapped`; `Auto` acts as `Normal` until
  CC detection) and the Hi-Z disconnect for the bench power-cut test (FW-15). Only the SWD-host uses them so far.
- 🧪 **API-5 `swd.mode`** (was `swd_mode`, moved in 0.2.0): `SwdMode::BitBang` (default) or `SwdMode::Spi` (FW-21);
  takes effect on the next SWD transfer. A mode from a newer host (read as `SwdMode::Unknown`, WireWeaver SW-47) is
  refused with `InvalidArgument`, the current one stays.
- 🧪 **API-7 `device.variant`** (was `board_variant`, moved in 0.2.0): ro property, `BoardVariant::Main` / `Plus` /
  `Unknown` (FW-22); also the variant of `device.board.BOARD`. A variant from a newer firmware reads as
  `BoardVariant::Other(n)` on the host (`future_version = Other`, SW-47).
- ✅ **API-8 API tree 0.2.0**: `donguru_api` 0.2.0 declares the whole v1 tree (design: tpm
  `prj/P2534-donguru/design/api-v1.md`): `device` (contract traits, `variant`, `settings`), `leds`, `meter`, `plug`,
  `header`, `sbu`, `inputs`, `hub`, `swd`, `uart[]`, `i2c[]`, `gpio[]`, `adc`, `debug`. Handlers in one module per
  level (`fw/donguru/src/ww/`); observed properties served by `poll_properties` in the server task; `debug` behind
  the `debug-api` cargo feature (on by default). Implemented: the moved items (API-4, API-5, API-7), `debug.led_set`
  and the `const` items (minimal: no build id, git or build time yet, API-9); every other item answers
  `Unimplemented`; `uart[0..2]`, `i2c[0..2]`, `gpio[0..2].pin[0..4]` are valid indices. Tested 15 Oct 2026 on B153A:
  `ww introspect` shows the tree, the moved items read and write, `plug.orientation` observed end to end with a
  generated client. Plug and header VBUS are modes (`plug.vbus_mode`, `header.vbus_sw_mode`: `ForceOff` /
  `ForceOn` / `Auto` with limits; `device.settings.plug_vbus_at_boot` replaces the `plug-vbus-on` feature once
  implemented, API-11). CMSIS-DAP `probe-rs` read of B129A, powered through `plug.vbus_mode` (`donguru power on`,
  API-11), tested 16 Oct 2026 on B153A + B129A in both cable orientations, 64 KB of flash at 1000 kHz, three reads each,
  all identical: `Normal` on v0.19.0 firmware in 1.59 s, `Swapped` with the cable flipped on v0.20.1 in 1.52-1.56 s;
  the other orientation gives an SWD protocol error each time, as it should. `plug.orientation` was set in RAM
  through the J-Link (the CLI's `swd orientation` is not implemented yet). `ww info` ends with an error from `ww` itself (double disconnect, WireWeaver).

- ✅ **API-9 Device contract**: `device.firmware` (`FirmwareInfo`, build id), `device.last_reset` (`LastReset`:
  cause, crash record, boot count, clear), `device.board`, `device.uid`, `device.label` (`UserInfo`: name 32 B and
  notes 256 B, RAM only until the settings store, API-19), `device.counters` (`ww_counters`: RAM and BKP buffers,
  used words, build id; `watch` not implemented, polling `read` covers USB), `leds` as `ww_indication` (mode:
  `OnlyEssential` keeps the power LED; brightness in five steps; `identify` blinks both LEDs, `Busy` while running;
  RAM only). Code: `fw/donguru/src/ww/device.rs`, `ww/leds.rs`, `src/led.rs`, `src/last_reset.rs`. With FW-11,
  FW-13. Tested 15 Oct 2026 on B153A: `ww info`, `ww report` and `ww counters` (WireWeaver `987f09a9`) read
  everything, the ELF found by build id (the log part waits for API-10); label, LED mode, brightness and identify
  from Python, identify seen as a full-brightness blink in the TIM14 / TIM15 compare registers, `Busy` while it
  runs. ^revB
- ✅ **API-10 defmt log over WireWeaver**: `ww_log::LogDefmt` at `device.log` through `ww_defmt_logger` (WireWeaver
  STD-26), RTT kept as a tee (channel `defmt`, probe-rs / `cnt run` unchanged); the log store a `PersistentRamRing`
  of 8 KiB in `.uninit` (DropOldest), its session started at boot or continued after a soft reset, so the log from
  before a host connects and from before a crash arrives when a host resumes it; `device.log.DEFMT` with encoding
  and build id; `debug.log_flood` for the lost-count test (`donguru_api` 0.2.1). Code: `fw/donguru/src/log.rs`,
  server task in `ww/mod.rs`; host test tool `donguru-core/examples/device_log.rs` until CLI-19. Tested 16 Oct 2026
  on B153A: the boot log over USB and RTT at once; `--flood 1000` gives 227 lines and "773 lost", RTT (probe-rs,
  blocking) all 1000; lines logged before a HardFault forced through the J-Link arrive after the reset, with the
  session continued and `last reset: Fault`. A live run arrives before the backfill, the tool prints in arrival
  order. See FW-26. ^revB
- 🧪 **API-11 Plug and header power**: `plug.vbus_mode` / `header.vbus_sw_mode` (`ForceOff`, `ForceOn { max_voltage }`,
  `Auto` with voltage, current and power limits, plug only) with the state in `plug.vbus` / `header.vbus_sw` (`Off`,
  `On`, `Fault`, `Tripped(..)`), `plug.vbus_cycle(off, release_sbu)` answered when VBUS is back on (deferred),
  `plug.data` (U701), `header.relay`; all observable. The guard runs on every ADC update (~15 ms) against the worst
  voltage and |current| of the update; a guarded mode is refused above `max_voltage`; trips and the AOZ1375 fault
  (FLTB PC14, two updates low) latch until the next mode write, which re-arms. Logic in `donguru_power` (host tests),
  pins and tasks in `fw/donguru/src/power.rs`, handlers in `src/ww/plug.rs` / `header.rs`; docs
  `docs/user/power.md`. Needs WireWeaver SRV-20 (deferred answers on nested levels). Tested 15 Oct 2026 on B153A +
  B129A (~10 mA load) at 5.2 V: on / off, refused at a lower `max_voltage`, OverCurrent and OverPower trips 8 ms
  after switching on (B129A's inrush peak 42-55 mA), header cut on a later rise above a lowered `max_voltage`,
  cycles of 100 / 250 / 300 / 500 / 2000 ms answered after the off time (host 103..2000 ms, firmware timer to the
  ms), `WrongState` in `ForceOff`, `InvalidArgument` above 60 s, PA5 / PA6 / PA9 / PD1 read high through the
  J-Link. Not tested: data cut seen in `lsusb` (B129A has no USB; needs a USB DUT on the plug), a U601 fault, above
  5.5 V with a PD charger, observe pushes (read back instead: the Python bindings don't observe yet). With FW-8,
  FW-14. ^revB
- 🧪 **API-12 power-meter**: `meter.reading` (V / I / P from one ADC window), `temperature`, `totals` +
  `reset_totals`, `window`, `deadband` (observers marked changed only beyond it), `calibration` (nominal until FW-9);
  `adc` as `ww_adc::Adc` with the named channels, raw code and value (`fw/donguru/src/ww/meter.rs`, `ww/adc.rs`,
  integration in `src/adc.rs`). Tested 15 Oct 2026 on B153A (no DUT power: plug VBUS needs API-11): reading 5.217 V / 6 mA
  idle, temperature 33.6 °C, totals grow (1.2 C and 6.2 J in 217 s, matches 5.5 mA at 5.2 V), observe `OnChange`
  quiet on an idle DUT (reading once, totals 1/s), `adc.read` of every channel but `sbu1` / `sbu2`. Under load, 16 Oct 2026 on B153A + B129A
  (`plug.vbus_mode` ForceOn 5.5 V): 5.217 V, 16 mA (idle offset 6 mA, so ~10 mA as B129A is known), 83 mW, `totals`
  grow 16.4 mC/s and 85 mJ per 1 s step, peak 39 mA at switch-on (window-averaged inrush, plug-power saw 42-55 mA),
  observe quiet under the steady load. Not compared with a DMM: the ~6 mA idle offset is what FW-9 calibration
  removes. FLASH 259.5 KiB. Open: `adc.sbu1` / `sbu2` answer
  `Busy` until the SBU level (API-14) owns PA0 / PA1; `window` is 0.9 .. 14.9 ms (one sequence up to one half DMA buffer of
  sequences), longer needs averaging across updates. A read of `io7` / `io9` / `io10` switches the ADC to the
  extended set and stays there (meter keeps working, sequence 0.62 -> 0.90 ms). With FW-3. ^revB
- 📋 **API-13 Button and switch**: observable button (with a press counter) and slide switch, counted event stream.
  With FW-4. ^revB
- 📋 **API-14 SBU owner and UART on SBU**: one function at a time on the plug SBU lines, `sbu.release`; `uart[0]` as
  `ww_uart::Uart` with the TX/RX flip from `plug.orientation` and a two-line monitor. With FW-7. ^revB
- 📋 **API-15 GPIO banks**: plug (SBU1, SBU2, A2, B2) and header (IO7..IO10) as `ww_gpio::Bank` with pin events.
  With FW-10. ^v1.0
- 📋 **API-16 I2C buses**: plug SBU (I2C1) and header (I2C3) as `ww_i2c::I2c`. With FW-10. ^v1.0
- 📋 **API-17 Hub and SWD state**: observable hub state and recoveries, `hub.recover`, observable SWD session; FW-18's
  spurious recoveries after a cold boot fixed. ^revB
- 📋 **API-18 Bring-up API**: MCU pins by port and number (refused list, no memory write), boot-time pin snapshot,
  hub SMBus registers, UCPD1/2 state and role, raw LEDs, peripheral register reads (P2534#14oct-1014). ^revB

- 📋 **API-19 Persistent settings**: `device.settings` as `rw` properties kept in flash by the new `ww_settings` store
  (sparse ShrinkWrap records, power-cut safe, 4 pages at the end of flash): label, plug VBUS at boot (replaces the
  `plug-vbus-on` feature), button action, meter window / deadband, UART default; a factory key for FW-9. ^revB
- 📋 **API-20 SBU UART as CDC-ACM**: the SBU USART also as `/dev/ttyACM*`; one owner at a time (ACM while the tty is
  open, else WireWeaver), the owner shown in `sbu.function`, WireWeaver gets `InUse`, `sbu.release` overrides. With
  FW-7. ^revB
- 📋 **API-21 USB NCM and embassy-net**: CDC-NCM network interface with a DHCP server, the same API over WebSocket and
  UDP, and a ~1 KiB http start page that loads the browser UI from our web server (cargo feature `net`). ^revB

- 📋 **API-22 WebUSB landing page**: WebUSB capability and landing-page URL on the WireWeaver interface (no new
  endpoints), so the https web UI (with WebGPU) reaches the dongle from Chromium. ^revB
- 💡 **API-23 Receptacle side as its own level (`host`)**: the computer / charger side has no level today (only
  `meter.vbus_in`). `host.cc` (who sources, Type-C current from Rp), `host.pd`: PD sink on UCPD1 with
  `source_caps` (PDOs incl. PPS), `request(pdo, voltage, current)`, the current contract, `last_source` (kept for
  FW-28) and a `messages` stream of decoded PD messages. Lab `charger-inspector`, `pd-sniffing`.
- 💡 **API-24 Plug CC role**: `plug.cc_role` = `Off` / `Sink` (Rd) / `Source { current }` (Rp 0.9 / 1.5 / 3 A) /
  `Auto` (follow the computer side, the default), replacing FW-17's hardcoded drive. Lets a DUT be a source (power
  bank, phone with reverse charging) and is what back-feeding needs: the guard opens the switch while both sides
  source. Lab `who-feeds-whom`.
- 💡 **API-25 Meter zero and averages with uncertainty**: `meter.zero()` learns the current offset while the plug is
  off (the INA181's offset is worth tens of mA on a 5 mΩ shunt), `meter.average(window)` returns mean / min / max and
  a one-sigma error. One ADC step is ~3 mA: the docs say what the meter can and can't resolve. Lab `standby-current`.
- 💡 **API-26 High-rate meter capture**: current and voltage at up to ~50 kSa/s into a RAM buffer, started by a
  trigger (power on, a current level, now) and read out as a stream (`ww_adc::AdcStream` or own); the regular
  sequence pauses while it runs. Lab `inrush`.
- 💡 **API-27 Cable e-marker**: read the e-marker of the cable on the receptacle with SOP' (needs VCONN from
  Donguru's side or a source that supplies it): current, voltage, speed, length, vendor. Lab `cable-check`.
- 💡 **API-28 UART over TCP on NCM**: with API-21, the SBU UART as a raw TCP port (2323, like ser2net), one more
  owner next to WireWeaver and ACM under the same one-owner rule. Lab `network-over-usb`.
- 💡 **API-29 `ext` level for user code**: a level in `DonguruApi` reserved for firmware forks (`ww_impl!(ext: Ext)`,
  empty in the official firmware), versioned apart from the rest, so a fork adds methods without changing
  `donguru_api`'s version and the official `dg` keeps working. `dg info` shows a firmware with an `ext` as custom.
  Lab `own-api-method`.
- 💡 **API-30 PD source on the plug**: UCPD2 as a PD source towards the DUT: offer a PDO list (from the receptacle's
  contract when bridging, or a scripted scenario), record the DUT's requests and timings, renegotiate, hard reset.
  Spec-conformant messages only. Labs `pd-both-sides`, `pd-edge-cases`.
- 💡 **API-31 CAN levels**: `can[]` as `ww_can_bus::Can` (0 = SBU / FDCAN1, 1 = A2B2 / FDCAN2): bitrate, FD data
  rate, frames as a stream with timestamps, send. Lab `can-on-the-plug`.
- 💡 **API-32 Receptacle as a source**: Rp and a PD source on UCPD1 towards the computer, so a power bank on the plug
  can charge the computer, with a PD data-role swap so the computer stays the USB host (else it loses `dg`). Rev A
  refuses Rp on UCPD1 (hw_b153a.md §14): rev B question. Lab `laptop-from-power-bank`.
- 💡 **API-33 Many clients without fights**: USB (through the local server, CORE-12) and NCM (API-21) clients at the
  same time, by design: every state change is pushed to all observers; a request carries its client's identity, so
  logs and events say who switched what; functions one client must drive alone (UART TX, an SWD session, a capture)
  are leased to one client and others get "held by X" instead of interfering; a lease ends with its client. Lab
  `many-clients`.

## Host library (`CORE`)

- ✅ **CORE-1 Device enumeration and selection** (`donguru-core/src/device.rs`): `list()` from the USB descriptors
  without opening (VID:PID `c0de:cafe` and API id `donguru_api`, sorted by path), `Selector` by index, serial (any
  case, unique prefix) or USB path (`5-1.2`), auto-select when exactly one; `Dongle::open` with the version check.
  The generated `donguru_api` client (`client.rs`) and a typed facade per level (`levels.rs`: device, settings, leds,
  meter, plug, header, sbu, inputs, hub, swd, uart / i2c / gpio by `Side`, adc, debug) with `Error::{Device (kind +
  typed error), Unimplemented, Timeout, Incompatible, Link}`; the B153A net names (`nets.rs`, hw_b153a.md §11);
  `vbus.rs` builds a `VbusMode` from plain volts/amps (`force_on`, `auto`), used by `dg power on` / `dg header
  vbus on` (CLI-11, CLI-16) so the CLI doesn't need `ww_si`'s constructors itself.
  Tested 15 Oct 2026 on B153A (omarchy-m1): `dg device list` finds it, `dg device info` reads `device.variant`
  (Main), board, firmware, UID.
- ✅ **CORE-2 Error type**: `thiserror` errors mapped to exit codes by the CLI (`donguru-core/src/error.rs`).
- ✅ **CORE-3 In-process mock instead of a transport trait** (`donguru-core/src/mock/`, feature `mock`): the generated
  `DonguruApi` server on `ww_device::Server` over wire_weaver_client's `in_process` transport, so tests run the real
  host stack (link setup, version check, timeouts, observe). Models the Donguru levels with the firmware's rules
  (VBUS guard, SBU ownership, refused debug pins); IO arrays and streams answer `Unimplemented`.
  `donguru-core/tests/mock.rs` covers every level, device errors, `Unimplemented`, observe and a stuck device.
- 📋 **CORE-4 Python bindings** over the Rust core (PyO3 + maturin, `uv add donguru`): `donguru.list()`,
  `donguru.open(selector)`, levels as attributes (`dg.power.on()`, `dg.meter.read()`, `dg.meter.watch()`,
  `dg.meter.record()` → pandas, `dg.inputs.events()`), SI values as floats. The labs use this shape. ^v1.0
- ✅ **CORE-5 SWD link memtest** (`donguru-core/src/swd_memtest.rs`): memtest-style patterns over any
  `WordMemory` (data bus walking 1/0, Barr address bus, solid, checkerboard, address-in-address, moving inversions
  per chunk, byte lanes, seeded xorshift64* random), mismatches counted and located (address, expected, read,
  flipped bits). Unit-tested against fake RAM with a stuck bit, an aliased address bit, swapped byte lanes and rare
  read flips. Feature `probe-rs` (`swd_memtest/probe.rs`): attach through any CMSIS-DAP v2 probe at a given speed,
  halt, save the region, run, restore, resume; probe-rs without its C HID backend.
- 🐛 **CORE-6 Mock's VBUS guard doesn't refuse, it trips**: `docs/user/power.md` and `donguru_power` (the firmware
  logic crate, `donguru_power/src/lib.rs`'s `Switch::set_mode`, tested in `refused_over_the_limit_changes_nothing`)
  both say writing a guarded mode while VBUS is already over `max_voltage` is `Refused` and changes nothing. The
  mock's `MockState::apply` (`donguru-core/src/mock/mod.rs`) instead always applies the write and reports
  `Tripped(OverVoltage)`, as `donguru-core/tests/mock.rs`'s `plug_vbus_guard_and_cycle` shows ("switching on above
  the limit trips"). Found while building CLI-11 / CLI-16: `dg power on` / `dg header vbus on` cope either way (they
  read back `vbus` after the write and fail the exit code unless it is `On`), but a CORE-3 test relying on the
  mock's `Set` to refuse would be testing the wrong thing. Fix: give the mock the same refusal `donguru_power` has.
- ✅ **CORE-7 ELF keeper: `dg` always finds the firmware ELF** (CLI-19's `device counters` / `device log` decode):
  `cnt_core::elf::resolve` already did the lookup (`WW_ELF_PATH`, then `~/.cache/ww/elf`, then `target/`) that
  `donguru-core/src/counters.rs` / `src/log.rs` call; `just fw-size`'s release build calls `cnt elf add "$elf"`
  after building. Upstream in `cnt` (0.5.0, proposed and built per `prj/P2534-donguru/design/elf-keeper.md`, lands
  as 0.5.1): `cnt run` (the actual day-to-day flash path, `cargo run` in `fw/donguru`) now caches the ELF it just
  flashed the same way; `cnt_core::elf` gained a config file (`~/.config/ww/config.toml`'s `[elf] dirs`, `~`
  expanded) for a directory synced between machines, read before the local ELF directory and `add`'s default
  target when one's set, so a flash on either dev PC publishes straight into it — declared as the mesh's `ww-elf`
  Syncthing share (`tpm` `area/ops/provision.toml`) between gpd-omarchy and omarchy-m1, approved and syncing on
  both (round-tripped a test ELF each way, ~10-15 s). Tested 16 Oct 2026 on B153A rev A: flashed `fw/donguru` via
  `cargo run` (the `cnt run` runner); the ELF landed in the shared directory and synced to the other PC within
  seconds; `dg device counters` and `dg device log`, **no `--elf`**, found it by build id and decoded real
  counter names and defmt log lines (`device log`'s own "malformed frame (wrong ELF?)" noise is a pre-existing
  API-10/FW-27 framing issue, not an ELF-finding one). The hardware check needed `donguru-core`'s crates.io
  `cnt_core = "0.5"` temporarily `[patch.crates-io]`d to the branch's local path, since `cnt` 0.5.1 isn't
  published yet; reverted after, nothing committed with it — once `cnt` lands this works with a plain
  `cargo update -p cnt_core`.
- 💡 **CORE-8 Dongle lock**: a host-side lock per dongle serial (a lock file / advisory lock taken by donguru-core),
  so two CI jobs don't drive the same dongle (exclusive functions are API-33's leases once
  CORE-12 lets several processes share it); `dg device lock --wait`, held for a command or a
  session. Labs `ci-runner`, `test-farm`.
- 💡 **CORE-9 pytest plugin** (`pytest-donguru`): a `dut` fixture (power-cycled device, console, meter), per-test
  energy and peak current in the report, console output kept on failure; `--donguru`, `--donguru-uart`. Lab
  `pytest-fixture`.
- 💡 **CORE-10 Remote dongles**: `dg serve` shares a PC's dongles over the network (WireWeaver over UDP / WebSocket
  through the host), `dg --remote HOST` and `donguru.connect(HOST)` use them like local ones. Needs authentication and
  an allow-list before it listens beyond localhost: security-sensitive, plan on `top`. Lab `test-farm`.
- 💡 **CORE-11 Dynamic calls**: call any method or property by its path through the firmware's introspection,
  without the generated client knowing it (`dg.call("ext.morse", text=...)`), for forks and newer firmware. Lab
  `own-api-method`.
- 💡 **CORE-12 Through the local WireWeaver server**: donguru-core (CLI, Python, GUI) connects to a running `ww serve`
  over IPC when there is one, and opens the USB interface itself only when there isn't, so any number of local
  programs use one dongle at once. The server itself is wire_weaver's (not built yet). Lab `many-clients`.

## CLI (`CLI`)

- ✅ **CLI-1 Command tree and global flags**: `info`, `device`, `usb`, `gpio`, `udev`, `config`, `completions`;
  `--device`, `--format text|json|jsonl`, `--color`, `-v` / `-q` (`donguru-cli/src/cli.rs`).
- ✅ **CLI-2 Colour theme** adapted to terminal capabilities (`src/theme.rs`, `examples/theme_preview.rs`).
- ✅ **CLI-3 Shell completions** (`donguru completions <shell>`).
- ⬜ **CLI-4 External subcommands**: clap captures unknown subcommands, but `src/commands/external.rs` only prints
  "not implemented": resolve `donguru-<name>` on `PATH` (with a near-match hint), export the resolved device and
  format (the doc comment says `DONGURU_CTX_*`, the design doc `DONGURU_DEVICE` / `DONGURU_OUTPUT`: pick one), exec
  it, and list plugins in `donguru help`.
- ⬜ **CLI-5 `gpio read / write / configure / watch`**: arguments parsed, prints "not implemented". Needs FW-10,
  API-3, CORE-1.
- 🧪 **CLI-6 `usb status / connect / disconnect`**: the plug's USB data switch (U701) over `plug.data`
  (`donguru-cli/src/commands/usb.rs`); exit code reflects the data path actually ending up where asked, not just
  that the call succeeded. Replaces the earlier `usb list / attach / detach / power / data` sketch (no per-port hub
  control in v1, see API-11's note). Snapshot-tested against the mock (CORE-3) and 16 Oct 2026 on B153A rev A
  (`status`, `connect`, `disconnect`). ^revB
- ✅ **CLI-7 `device list / info / select`, `info`**: `device list` / `device info` (text / json / jsonl,
  `donguru-cli/src/commands/device.rs`), tested 15 Oct 2026 on B153A. `device select` resolves the selector against
  the connected dongles and pins the serial in `./donguru.toml` (`donguru-cli/src/project_config.rs`), read back as
  the lowest-priority layer in `Context::from_cli` (before CLI-9's full layering). Top-level `info` is `device
  info`'s summary (now with `last_reset`) under the short, no-noun spelling. Tested against the mock
  (`info_from_the_mock`, `project_config::tests`) and 16 Oct 2026 on B153A rev A: `info` and `device info` (text and
  json) agree, `device select 0` pins the serial in `donguru.toml` and is picked up as the device default on the
  next invocation, `device info --device serial:<bogus>` exits `NoDevice` (3), an unknown subcommand exits
  `BadArgs` (2).
- 🚧 **CLI-8 `udev`**: `generate` writes the rule (uaccess / group / all); `install`, `uninstall`, `status` are stubs.
- ⬜ **CLI-9 `config generate / show / path / validate`**: stub. Layering per the design doc (defaults, system,
  user, project file, `DONGURU_*`, flags).
- 📋 **CLI-10 `fw version` / `fw update`**: in the design doc, not in the command tree yet. Needs FW-11, FW-12.
- ✅ **CLI-11 `power status / on / off / cycle / meter / watch / totals`** (`donguru-cli/src/commands/power.rs`):
  `status` tables plug + header + meter in one read; `on` (`--allow-voltage`, `--trip`) and `off` read back `vbus`
  after the write and fail the exit code unless it ended up where asked (`On` / `Off`), covering a trip or a latched
  fault without the host needing its own copy of the guard; `cycle --off --release-sbu`; `meter` one reading;
  `totals [--reset]` in mAh / mWh; `watch [--interval | --on-change]` streams `meter.reading` through CLT-28 observe
  until Ctrl-C (`--format json` is refused: use text or jsonl). `donguru_core::vbus` builds the mode, and `cycle`'s
  `off` time, as integer millivolts/milliamps/milliseconds (the firmware's `ww_si` has no `f32`: a float-encoded
  value sent to it doesn't decode at all, `PropertyDesFailed` / `ArgsDesFailed` — found testing on B153A, fixed
  before anything shipped with the bug). Snapshot-tested against the mock (CORE-3) and 16 Oct 2026 on B153A rev A
  without a DUT: `status`, `on`/`off` (plug VBUS actually switches, meter confirms ~16 mA draw), `cycle --off 300ms`,
  `meter`, `totals` and `totals --reset`, `watch --interval` streaming real readings, text/json/jsonl. Not tested: a
  trip or `Auto`'s current/power limits under real load (needs a DUT). ^revB
- ✅ **CLI-16 Header VBUS output with a clear warning** (`donguru-cli/src/commands/header.rs`): `header vbus on`
  always prints the present receptacle voltage (from `meter.reading`, or just the warning if the meter isn't
  implemented yet) and a warning that VBUS_SW is the computer's own VBUS, not a regulated 5 V supply; going above
  the 5.5 V default needs `--allow-voltage`, which is the explicit confirmation (same flags as CLI-11's `power on`).
  `header relay on / off / status` (U101). Same warning text still needed in the GUI, Python and docs (only
  `docs/user/power.md` has it so far). Snapshot-tested against the mock and 16 Oct 2026 on B153A rev A: `vbus
  status/on/off` (the warning shows the real measured voltage), `relay status/on/off`. ^revB
- 📋 **CLI-12 `i2c scan / read / write` and `uart`**: in the design doc, not in the command tree yet.
- 🐛 **CLI-13 Exit codes differ from the notes**: `src/exit.rs` has device error = 4, timeout = 5 and no transport
  code; `developer-notes.md` has transport = 4, device = 5, timeout = 6. Decide before anything ships.
- 📋 **CLI-14 `--version` with git SHA and build time** (`build.rs`), per the version rule in AGENTS.md.
- 📋 **CLI-15 Output renderer**: commands return typed values, one renderer for text / json / jsonl
  (`developer-notes.md`).
- ⬜ **CLI-17 `swd orientation / release`**: arguments parsed, prints "not implemented" (`src/commands/swd.rs`),
  same as the other device commands: needs CORE-1, API-4. CMSIS-DAP itself (FW-15) is not a `donguru` subcommand,
  probe-rs talks to it directly over USB.
- ✅ **CLI-18 `swd memtest`**: CORE-5 from the command line, `--chip` (probe-rs name), `--probe` (Donguru by
  default), `--speed` list in kHz, `--base`/`--size` (default the chip's largest RAM), `--pattern`, `--seed`,
  `--passes`, `--no-restore`; text with colour and a spinner, `json`/`jsonl`; exit 1 on any mismatch. Tested
  12 Oct 2026 on B153A + B129A (STM32G0B1).

- ✅ **CLI-19 `device label / log / counters / reset-cause`**: the device contract from `dg` (API-9, API-10).
  `device label [NAME] [--notes]` gets/sets `UserInfo` name and notes. `device reset-cause [--clear]` shows
  `LastReset`'s cause, message, boot count and crash record, `--clear` forgets it. `device counters [--watch]` and
  `device log [--follow]` decode `ww_counters` / `ww_log` with the firmware's ELF found by its build id
  (`cnt_core`, `defmt-decoder`; `donguru-core/src/counters.rs`, `src/log.rs`), the same mechanism `ww counters` /
  `ww report` use; a device with nothing to decode (used words 0, no log session) needs no ELF at all. Mock: models
  `last_reset` / `boot_count` / `clear` and label notes; `counters`/`log`'s own RPCs (`read`, `frames`) still answer
  `Unimplemented` (as the real firmware does until API-10 and more counters land), so only the "nothing yet" path
  is mock-tested for those two; real decoding needs hardware and firmware that has `cnt`/defmt counters to show.
  Tested against the mock (`donguru-core/tests/mock.rs`: `reset_cause_and_clear`,
  `counters_report_with_nothing_to_decode`, `log_dump_before_the_device_ever_logged`; CLI renderers via insta
  snapshots), and 16 Oct 2026 on B153A rev A: `device label` gets and sets name/notes (text and json);
  `device reset-cause` decoded a real stored crash record (`HardFault`, boot count, crash uptime, stack dump) left
  by an earlier session's board use, and `--clear` forgot it (boot count and crash fields back to zero on the next
  read); `device log` took the "nothing yet" path (API-10 not built) with an empty log, exit 0; `device counters`
  found a non-zero build id and reached the ELF-by-build-id lookup for real, failing with the documented "no ELF
  with that build id in the cache or target/" message since no firmware ELF was cached — the happy decode path
  (ELF present) is still untested, needs a firmware build whose ELF is kept around (`cnt elf add` or `target/`) at
  test time. ^revB
- 📋 **CLI-20 `sbu` and `uart`**: `sbu status / release / orientation`, `uart open / monitor / pty / break` (API-14). ^revB
- 📋 **CLI-21 `led mode / brightness / identify`** (API-9).
- 📋 **CLI-22 `adc`, `input`, `hub`**: `adc read`, `input status / watch`, `hub status / recover` (API-12, API-13,
  API-17).
- 📋 **CLI-23 `debug`**: pins by net name or port pin, boot-state diff, hub registers, UCPD, register reads (API-18).
  ^revB
- 💡 **CLI-24 `dg top`**: one live screen of every observed level.
- 💡 **CLI-25 `dg cc` and `dg pd`**: `cc status` (role and Type-C current on both sides), `cc plug --role
  source|sink|off|auto` (API-24); `pd caps | request V | watch | last` on the receptacle side (API-23). Labs
  `who-feeds-whom`, `charger-inspector`.
- 💡 **CLI-26 `dg usb cycle`**: `--off DURATION`, `--with-power` (data and VBUS together, a real unplug), `--wait
  VID:PID` (until the OS lists the device again; prints the time it took, exit code 5 on timeout). The host-side
  wait is donguru-core, the timing the firmware's. Lab `usb-unplug`.
- 💡 **CLI-27 `dg input on press|release|long-press|switch -- CMD`**: runs a host command on a button or switch
  event while it runs, prints each event and the command's exit code (API-13). Lab `button-shortcut`.
- 💡 **CLI-28 Power direction**: `dg power meter` / `watch` show `→ DUT` / `← DUT` next to the signed current, json
  gets `"direction"`. Lab `who-feeds-whom`.
- 💡 **CLI-29 `dg power log | plot | record`**: `log FILE --interval --for` (append, backfill from FW-29 after a
  sleep, `--summary`), `plot FILE -o PNG` (V / I / P over time, a summary line), `record --rate --for --trigger`
  (API-26). Labs `overnight-log`, `inrush`.
- 💡 **CLI-30 `dg power zero` and `meter --average DURATION`** with min / max and the error (API-25). Labs
  `standby-current`, `cable-check`.
- 💡 **CLI-31 `dg cable info`**: the receptacle cable's e-marker (API-27). Lab `cable-check`.
- 💡 **CLI-32 `dg power on --report`**: peak current, time to settle and the settled current after switching on,
  from the meter's readings around the switch (API-12; finer with API-26). Lab `safe-first-power-on`.
- 💡 **CLI-33 `dg watchdog`**: host side `--check CMD --every --misses --off` (cycle the DUT when a check keeps
  failing, jsonl log of checks and cycles), and `watchdog device --pin | --uart --timeout --grace` to arm FW-31.
  Lab `hung-board`.
- 💡 **CLI-34 `dg power measure -- CMD`** (or `--for DURATION`): meter exactly the span of a command: time, energy,
  charge, peak, mean; `--repeat N`; the command's exit code passes through. Labs `energy-per-operation`,
  `ci-runner`.
- 💡 **CLI-35 `dg uart expect PATTERN`**: wait for a line or regex on the UART with `--timeout`, `--fail-on PATTERN`,
  `--log FILE`; exit 0 / 1 / 5 (timeout). Also in Python as `console.expect()`. Labs `power-cut-test`,
  `ci-runner`.
- 💡 **CLI-36 `dg each PATTERN -- COMMAND`** and `device list --tree`: run one command on every dongle whose label
  matches, in parallel, a result line each, non-zero if any failed; the tree shows dongles by hub and port. Lab
  `test-farm`.
- 💡 **CLI-37 `dg seq load | run | result`**: sequences (FW-33) from a TOML file, run now or standalone, results as
  csv / json. Lab `voltage-sweep`.
- 💡 **CLI-38 `dg trace`**: the DUT's RTT / defmt log (FW-34) and the meter on one timeline: `--elf`, `--for`, a jsonl
  record, `trace view` in the terminal or as HTML. Lab `logs-and-current`.
- 💡 **CLI-39 `dg call PATH ARGS`**: dynamic call of any firmware item (CORE-11), json in and out. Lab
  `own-api-method`.
- 💡 **CLI-40 `dg probe pins` and `dg uart detect`**: measure each plug pin floating / with a weak pull-up / with a
  weak pull-down and name what it sees (CC role, idle UART, pull-ups); try common baud rates on a toggling line
  until text is readable. Lab `explore-an-accessory`.
- 💡 **CLI-41 `dg can open | dump | send`** (API-31), candump format, `--socketcan` on Linux; a python-can
  interface `donguru`. Lab `can-on-the-plug`.
- 💡 **CLI-42 `dg la capture`** (FW-37): channels, rate, trigger, pre-trigger, sigrok `.sr` output for PulseView.
  Lab `logic-analyzer`.
- 💡 **CLI-43 `dg soak`**: a weighted random sequence of events (power cycle, USB cycle, pulse, idle) from a TOML
  file with a check after each, `--seed`, `--for`; `soak report`, `soak replay --from N`. Lab `soak-test`.
- 💡 **CLI-44 `dg pd scenarios list | run`** (API-30): spec-conformant source scenarios against a DUT's PD sink, an
  HTML report. Lab `pd-edge-cases`.
- 💡 **CLI-45 `dg sbu usb on | off`**: Plus variant, the SBU pair to hub port 4 (`sbu.route_usb`, API-14). Lab
  `usb-over-sbu`.
- 💡 **CLI-46 `dg clients`**: who is connected (USB through the server, NCM), what they observe and which leases they
  hold (API-33). Lab `many-clients`.

## GUI (`GUI`)

- 💡 **GUI-1 `dg ui`**: native egui app over USB (the same donguru-core): a dongle list with labels and identify, a
  Power tab (a switch per output, cycle with off time, guard limits), a Meter tab (live V / I / P plot, totals with
  reset), a PD tab (offers as cards, message timeline). Labs `hello`, `phone-charging`, `kill-switch`,
  `charger-inspector`. The browser UI over NCM / WebUSB (API-21, API-22) is the same app built for wasm.
- 💡 **GUI-2 Trace timeline**: current plot over log lines on one time axis (CLI-38's data), hover a spike to see
  the line before it. Lab `logs-and-current`.
- 💡 **GUI-3 Farm view**: dongles (local or through `dg serve`) as a grid of tiles: label, power, last console line,
  console and power-cycle buttons. Labs `rack-console`, `test-farm`.

## Packaging and docs (`PKG`)

- ✅ **PKG-1 Documentation site**: Zensical, quickstart / usage / scripting / design pages, published by
  `.github/workflows/docs.yml`. User pages are mostly placeholders until commands work.
- ✅ **PKG-2 CI and hooks**: `just verify` in GitHub Actions, prek pre-commit / pre-push hooks.
- 📋 **PKG-3 Packages**: `.deb` / `.rpm` first, then Homebrew and Chocolatey; man pages, completions and the udev rule
  installed by the package (`docs/design/packaging/`). ^v1.0
- 🚧 **PKG-5 Lab book**: 48 labs in six levels (first hour, bench, automation, developer, hacker, insane), one Typst
  file each in `labs/` (goal, setup, steps in CLI / Python / GUI, what you should see, what next), built into one
  PDF (`just labs-book`) and HTML pages with per-lab PDFs (`just build-labs`, Typst 0.15 bundle export,
  experimental). Labs describe Donguru as it should be: each lists the feature IDs it relies on, the header shows
  their status read from FEATURES.md, and the book ends with a feature map, missing first. Most features they use
  are 💡 items added for them. Missing: photos, real output from a rev B board, publishing (docs CI needs Typst and
  the vhrd package, P2534#17oct-1150). Catalog and findings: tpm `prj/P2534-donguru/design/lab-book*.md`.
- 📋 **PKG-4 License**: the workspace says `license = "TBD"`. ^v1.0
- 💡 **PKG-6 SBU breakout**: a small USB-C receptacle board with SBU1, SBU2, CC, VBUS, GND (and D+/D-) on 2.54 mm
  pins and labels, for wiring Donguru's SBU functions (UART, I2C, SWD, GPIO) to a board without Donguru's SBU
  convention; sold with it or as an accessory. Labs `serial-console`, `i2c-on-the-plug`.
- 💡 **PKG-7 GitHub Action** (`vhrdtech/donguru-action`): installs `dg` on a self-hosted runner, takes the dongle's
  lock (CORE-8) for the job, uploads the console and power numbers as artifacts. Lab `ci-runner`.
- 💡 **PKG-8 SBU convention for DUT boards**: a docs page and a reference design (mux, CC debug-accessory detection,
  ESD, KiCad snippet) for products that want SWD and a UART on their USB-C port with Donguru as the other end;
  generalizes B129A's SWD_SEL circuit. Lab `sbu-convention`.
- 💡 **PKG-9 Pigtails**: a short cable for rev B's plug pads with both CC lines (a cut regular cable carries only one,
  so no debug accessory mode), as an accessory and possibly a Donguru with one soldered on from the start. Lab
  `broken-plug-bypass`.

## Dropped and superseded

- **API-2 `led_on` / `led_off`** (status LED, the template's example methods): replaced in 0.2.0 by `debug.led_set`
  (raw LEDs, API-8) and `leds` (`ww_indication`, API-9).
