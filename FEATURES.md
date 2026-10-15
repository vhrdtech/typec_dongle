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
- 📋 **FW-8 Power switch and USB data switches** per downstream port, with safe defaults at boot. ^revB
- 📋 **FW-9 Power meter calibration**: offsets and gain per board, stored in the config page. ^v1.0
- 📋 **FW-10 GPIO and I2C** on the external pins. ^v1.0
- 📋 **FW-11 `fw_info` / `hw_info`** (embedded_bedrock device contract): firmware name, version, git SHA, build
  time; board revision and serial. ^revB
- 📋 **FW-12 Firmware update without a probe**: STM32 system bootloader via the boot switch first, then a custom
  bootloader; the CLI side is CLI-10. ^v1.0
- 📋 **FW-13 HardFault handling for production**: reset instead of looping (TODO in `HardFault`); the backup-register
  counter already records the fault.
- ✅ **FW-15 SWD-host over the plug SBU lines**: CMSIS-DAP v2 (bulk, `src/cmsis_dap.rs`) on a composite USB device
  alongside WireWeaver (`src/ww.rs`), carrying `dap_rs`'s command processor over a GPIO bit-bang SWD driver
  (`src/swd.rs`: SWCLK/SWDIO on PA1/PA0, the SBU side of the TXS0102 level shifter, orientation-swappable, SWCLK
  clamped 10 kHz..4 MHz since FW-20, default 400 kHz). R211/R212 (the shifter bypass to PB8/PB6) are not fitted on B153A, so
  the driver goes around the shifter: `SBU_OE` (PB2) and `SBU_PU_EN` (PC15) always low, V_SBU at 3.3 V (PD3 high),
  so 3.3 V targets only. No hardware reset line (NRESET/NTRST): `SWJ_Pins` reports it as never asserted. JTAG and
  SWO not wired (SWD only). Orientation is a runtime setting (`DonguruApi::swd_orientation`, API-4), not
  auto-detected from CC polarity. The SWD-host pins go Hi-Z on a clean `DAP_Disconnect` and also when the CMSIS-DAP
  v2 endpoints get disabled (USB reset/suspend, probe-rs or its process going away without disconnecting
  cleanly), not just on an explicit `DonguruApi::swd_release` (`src/cmsis_dap.rs::dap_task`): a bench harness only
  has to end the probe-rs session, one way or another, before cutting the DUT's power.
  Tested 12 Oct 2026 on B153A + B129A (CANnify Micro, STM32G0B1) with probe-rs: reads at 100 kHz, 400 kHz and
  1 MHz (16 KB in about 2 s), both cable orientations (flipped = `Swapped`), pins Hi-Z after disconnect.
  Next: a level-shifter mode for 1.8 V targets (P2534#12oct-1338b); SPI-driven SWD is FW-21.
- ✅ **FW-17 Plug CC drive for SWD-host**: stopgap, hardcoded (`src/swd.rs`, `src/cmsis_dap.rs`; P2534#12oct-1338b).
  PD0/PD2 (UCPD2_CC1/CC2; B153A has no DPO2036, hw_b153a.md §0, so these reach the plug CC directly) are driven
  push-pull high on a successful `DAP_Connect` and released to Hi-Z together with SWCLK/SWDIO on `DAP_Disconnect`,
  CMSIS-DAP v2 endpoint disable and `swd_release` — a downstream board such as B129A gates its SBU->SWD mux on CC
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
    task goes back to listening, so `swd_release` is never stuck behind a host that stopped reading.

  Hi-Z on `DAP_Disconnect`, on endpoint disable and on `swd_release` is unchanged.
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
  `swd_release`) takes `SBU_OE` low first (its 10K pull-ups on both sides go with it), then PB6/PB8/PB11 Hi-Z and
  SPI2 off; switching back to bit-bang does the same. Selected at run time with `swd_mode`, or at boot with the
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
  (`DonguruApi::board_variant`, API-7, FW-11 hw_info). Tested 14 Oct 2026 on B153A (J-Link via omarchy-m1,
  P2534#14oct-1439): reads PA10 high → `BoardVariant::Main` on 2 cold power-cycles (true replug, J-Link VTref
  0.00 V between them) and 2 debugger-forced resets, hub attaching correctly every time. 🧪 because Plus is
  untested hardware.
- 🐛 **FW-23 Firmware not clippy clean**: `cargo clippy --bins --examples -- -D warnings` in `fw/donguru` fails with
  13 lints (doc list indentation in `hub.rs` / `swd.rs`, operator precedence in `led.rs`, an empty `loop {}` in
  `main.rs`, unused `ChannelSet::Extended`, `adc::set_channels` and `Brightness::Dim`); `just clippy` covers only
  the host workspace, so nothing catches them. Found 15 Oct 2026.
- 🚧 **FW-24 WireWeaver USB throughput** (P2534#15oct-1747): was ~30% of USB FS bulk (`ww usb-loopback`, 1 KB
  messages: 152 round trips/s, 373 kB/s host -> device, 316 kB/s device -> host). Done: flash prefetch on
  (`main.rs`, embassy-stm32 0.6 leaves it off on the G0); wire_weaver TR-12 (U2Head byte fast path, replies
  flushed when no request waits). Pending outside this repo: double-buffered bulk endpoints, which need
  embassy-stm32 with usb_v4 double buffering (patch written, not upstream yet) and an embassy-usb release with
  `endpoint_bulk_*_double_buffered`; measured with that patch: 1 KB 330 round trips/s, 697 kB/s host -> device,
  489 kB/s device -> host. Tested 15 Oct 2026 on B153A.

## API (`API`)

- 📋 **FW-14 Switched VBUS on the header** (J101 pin 10, PD1 `5V_EN`, net to be renamed VBUS_SW): it is the computer's
  VBUS as it is, 5 V or up to 20 V after PD. Off at boot; report the receptacle voltage (PA3) with the state; turn it
  off when VBUS changes unless the user allowed a higher voltage. ^revB
- 🧪 **API-1 WireWeaver over USB**: `DonguruApi` served by `fw/donguru/src/ww.rs` (USB FS, VID:PID `c0de:cafe`, API
  id in a string descriptor for `ww list`). Built against the local wire_weaver checkout; not yet tested with
  `ww list` on hardware.
- ✅ **API-2 `led_on` / `led_off`**: status LED only, the template's example methods.
- 📋 **API-3 Full API**: ports (power, data), power meter readings and streams, button / switch events, USART bridge,
  GPIO, I2C, device info; designed together with the CLI command tree. ^revB
- 🧪 **API-4 `swd_orientation` / `swd_release`**: SWD-host cable orientation (rw property) and the Hi-Z disconnect
  for the bench power-cut test (FW-15). `donguru_api` 0.1.1.
- 🧪 **API-5 `swd_mode`**: rw property, `SwdMode::BitBang` (default) or `SwdMode::Spi` (FW-21), next to
  `swd_orientation`; takes effect on the next SWD transfer. Additive, `donguru_api` 0.1.2: older hosts keep working.
  A mode from a newer host (read as `SwdMode::Unknown`, wire_weaver SW-47) is refused, the current one stays.
- 🧪 **API-7 `board_variant`**: ro property, `BoardVariant::Main` / `Plus` / `Unknown` (FW-22), for host tooling
  and future `hw_info` (FW-11). Additive, `donguru_api` 0.1.3: older hosts keep working.
  A variant from a newer firmware reads as `BoardVariant::Other(n)` on the host (`future_version = Other`, SW-47).

## Host library (`CORE`)

- ⬜ **CORE-1 Device enumeration and selection**: `list()` is `todo!()`, `select` resolves nothing yet
  (`donguru-core/src/device.rs`). Selectors: serial, index, USB path; auto-select when exactly one dongle is present.
  ^revB
- ✅ **CORE-2 Error type**: `thiserror` errors mapped to exit codes by the CLI (`donguru-core/src/error.rs`).
- 📋 **CORE-3 Transport trait with a mock**: so commands can be tested without hardware (design doc, *Transport
  model*).
- 📋 **CORE-4 Python bindings** over the Rust core. ^v1.0
- ✅ **CORE-5 SWD link memtest** (`donguru-core/src/swd_memtest.rs`): memtest-style patterns over any
  `WordMemory` (data bus walking 1/0, Barr address bus, solid, checkerboard, address-in-address, moving inversions
  per chunk, byte lanes, seeded xorshift64* random), mismatches counted and located (address, expected, read,
  flipped bits). Unit-tested against fake RAM with a stuck bit, an aliased address bit, swapped byte lanes and rare
  read flips. Feature `probe-rs` (`swd_memtest/probe.rs`): attach through any CMSIS-DAP v2 probe at a given speed,
  halt, save the region, run, restore, resume; probe-rs without its C HID backend.

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
- ⬜ **CLI-6 `usb list / attach / detach / power / data`**: stub. Needs FW-8, API-3. ^revB
- ⬜ **CLI-7 `device list / info / select`, `info`**: stub. Needs CORE-1. ^revB
- 🚧 **CLI-8 `udev`**: `generate` writes the rule (uaccess / group / all); `install`, `uninstall`, `status` are stubs.
- ⬜ **CLI-9 `config generate / show / path / validate`**: stub. Layering per the design doc (defaults, system,
  user, project file, `DONGURU_*`, flags).
- 📋 **CLI-10 `fw version` / `fw update`**: in the design doc, not in the command tree yet. Needs FW-11, FW-12.
- 📋 **CLI-11 `power on / off / status / meter / watch`**: in the design doc, not in the command tree yet. ^revB
- 📋 **CLI-16 Header VBUS output with a clear warning**: enabling the header's switched VBUS (FW-14) shows the present
  voltage and a warning that it is the computer's VBUS, not a regulated 5 V; above 5.5 V it needs an explicit
  confirmation flag. Same warning in the GUI, Python and docs. ^revB
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

## Packaging and docs (`PKG`)

- ✅ **PKG-1 Documentation site**: Zensical, quickstart / usage / scripting / design pages, published by
  `.github/workflows/docs.yml`. User pages are mostly placeholders until commands work.
- ✅ **PKG-2 CI and hooks**: `just verify` in GitHub Actions, prek pre-commit / pre-push hooks.
- 📋 **PKG-3 Packages**: `.deb` / `.rpm` first, then Homebrew and Chocolatey; man pages, completions and the udev rule
  installed by the package (`docs/design/packaging/`). ^v1.0
- 📋 **PKG-4 License**: the workspace says `license = "TBD"`. ^v1.0

## Dropped and superseded

(none yet)
