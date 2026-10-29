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
  brightness (`src/led.rs`). B153A. Animation, math and driver loop from embedded_bedrock's `bedrock_led` (DEV-12,
  shared with rePort); `src/led.rs` sets up TIM14 / TIM15, `ww/leds.rs` takes the identify bookkeeping and the
  brightness peaks from `bedrock_led::indication`. What they show is the common bedrock look (`bedrock_led::look`,
  DEV-15): power `OK`, status `HEARTBEAT`.
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
  21 Oct 2026 (FW-46): B129A on the plug enumerated at full speed behind the port 2 swap, so the swap is right on
  B153A (unswapped it comes up low-speed).
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
- ✅ **FW-12 Firmware update without a probe**: through the STM32 ROM bootloader (USB DFU), decided by Roman 21 Oct
  2026 as B135 did (BOOT-9, BOOT-10). `device.boot.reboot(Updater)` (API-58) writes a request into the boot word
  (TAMP_BKP4R, the fifth backup register; `bkp_cnt!` keeps the four below) and resets; first thing in `main`
  (`fw/donguru/src/boot.rs`) the next boot clears it and jumps to the ROM at 0x1FFF0000 from the reset state. The
  ROM's leave jumps back with its VTOR and an interrupt enabled: the boot sees it (`Entered::from_reset`: SWS, USB
  clock, NVIC, VTOR; on B153A the ROM left SWS on HSISYS, VTOR 0x1FFF0000 and one IRQ gave it away) and resets
  once. The hub stays up through all of it on rev A because RTC_OUT1 drives its RESET_N on PC13 (`src/hub.rs`
  `ResetPin`, RTC_CR OSEL alarm A + POL; feature `rev_a`, FW-47): the backup domain keeps the level through a system
  reset. Since FW-46 the bring-up keeps a hub that held through the reset instead of pulsing it. The USB
  serial is the ROM's form for this chip (`donguru_board::boot::rom_serial`), so `ww` follows the board into the
  ROM and back. Pure logic in `donguru_board::boot` (host tests). `just update SERIAL`, user page
  `docs/user/firmware-update.md`. Tested on B153A (omarchy-m1, 21 Oct 2026): reboot -> 0483:df11 behind the hub in
  under 0.5 s -> `ww dfu stm32 flash --wait 10 --follow` 415 KB written and verified in 42.5-42.9 s -> firmware back,
  5 times in a row, no power cycle; `reboot --into app` too. Not covered: a firmware that does not start (FW-45),
  BOOT0 (nBOOT_SEL = 1 on B153A, change_b153b.md). ^v1.0
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
  I2C probe that reads the hub's VID/PID back all-zero twice (a reset hub waits for its config again; a NAK only
  means busy, FW-41) — and redoes the reset + config + attach bring-up. No probing for 1 s after an attach, and a
  USB-side request within 2 s of one is the echo of our own hub reset, not a new loss.
  Tested 12 Oct 2026 on B153A (P2625 bench, omarchy-m1): the bench's VBUS_SW line now has a 10R series resistor
  (power-cut-bench session) that stops the original inrush from browning out the hub, so this was verified by a
  debugger-forced hub reset instead (same electrical event as the brown-out, from the hub's point of view): a
  J-Link (`--probe 1366:0105`) wrote PC13 (GPIOC BSRR, `0x5000_0818`) low then high while the firmware kept
  running, 10 times. Donguru's composite device (VID:PID `c0de:cafe`) re-enumerated on the host every time, about
  2.0-2.3 s after each pulse (`journalctl -k`: the hub itself, VID:PID `0424:2512`, re-attaches first). The ~1 s
  target in the tpm task was a rough estimate; ~2 s here is mostly host-side USB re-enumeration overhead, not the
  firmware's own recovery time. Not yet exercised against a real brown-out (needs the 10R resistor bypassed, which
  the user declined for now since the bench is in use for cut measurements).
  Fixed 20 Oct 2026 (FW-41): the I2C probe was a zero-length write that took a NAK for a loss, so it reset the
  hub one or two times 200-600 ms after every boot (the hub NAKs while it attaches and while the host enumerates
  it), and for good while the MCU's own device stayed off the bus. Rechecked on B153A: no recovery at boot
  (`cnt run`, 3 resets), one per PC13 pulse, USB back each time.
  Rechecked 21 Oct 2026 with the FW-46 build: since FW-12 RTC_OUT1 drives PC13 on rev A, so the pulse now goes
  through RTC_CR (J-Link: RTC_WPR 0xCA, 0x53, RTC_CR 0x0020_0000 then 0x0030_0000, RTC_WPR 0xFF; GPIOC BSRR no
  longer reaches the pin). 3 pulses, each found by the I2C probe (all-zero) and recovered about 0.2 s later, hub and
  Donguru back on the host within 1 s.
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
  `donguru_board::hub`, P2534#14oct-1439). Reading the hub's own power-on-default VID/PID/DID (HUB-5) before
  configuring it does not work on this hub's SMBus-slave strap — it reads all-zero regardless of delay (tried
  0/5/100/1000 ms), confirming hw_b153a.md §9's existing caution that these registers hold whatever was last
  written there, not a factory default. The signal used instead: PA10 (`SBU_TO_USB`, U901 S) carries R902, a
  10K pull-down fitted only on Plus (hw_b153a.md §9/§11); read against the MCU's own ~40k pull-up after
  releasing PA10's UCPD1 dead-battery pull-down (`SYSCFG.cfgr1().ucpd1_strobe`, resolves the 🔍 open in
  hw_b153a.md §11), high = Main, low = Plus. `donguru_board::hub::hub_config` then builds the hub's config
  (`usb251xb::Variant`, port count, D+/D- swap mask for FW-6) from the detected variant instead of a hardcoded
  Main constant; the pure mapping and config builder are host-unit-tested for both variants (`donguru_board::hub`,
  `just test`), including the not-yet-built Plus. Variant exposed read-only over WireWeaver
  (`device.variant`, API-7, FW-11 hw_info). Tested 14 Oct 2026 on B153A (J-Link via omarchy-m1,
  P2534#14oct-1439): reads PA10 high → `BoardVariant::Main` on 2 cold power-cycles (true replug, J-Link VTref
  0.00 V between them) and 2 debugger-forced resets, hub attaching correctly every time. 🧪 because Plus is
  untested hardware.
- ✅ **FW-23 Firmware clippy clean**: `just lint` runs `just fw-lint` (`cargo fmt --check` and
  `cargo clippy --bins --examples -D warnings` in `fw/donguru`, for the default features, `--all-features` and
  `--no-default-features`), so a lint or a feature-gated path that stops building fails the lint. The 13 lints of
  15 Oct 2026 were gone by 24 Oct but for the doc list indentation in `hub.rs` / `swd.rs` and `uart::replay` dead
  without `uart-replay`; the `usb_serial` example follows embassy main's API (`Usbsel::Hsi48`, `Driver::new`
  interrupt last). Fixed 24 Oct 2026.
- 🚧 **FW-24 WireWeaver USB throughput** (P2534#15oct-1747): was ~30% of USB FS bulk (`ww usb-loopback`, 1 KB
  messages: 152 round trips/s, 373 kB/s host -> device, 316 kB/s device -> host). Done: flash prefetch on
  (`main.rs`, embassy-stm32 0.6 leaves it off on the G0), off again since FW-38 (ES0548); release `opt-level = 3` instead of "z" (~40% faster
  here); WireWeaver TR-12 (U2Head byte fast path, replies flushed when no request waits); double-buffered bulk
  endpoints (`src/ww/mod.rs`, `WireWeaverClass::with_endpoints`) from embassy-rs/embassy#7275, merged; embassy from
  upstream main at a pinned rev (`fw/donguru/Cargo.toml` `[patch.crates-io]`, 19 Oct 2026; before: the romixlab/embassy
  `donguru-0.6-double-buffer` fork). Release
  build now: 1 KB 335 round trips/s, 664-676 kB/s host -> device, 520 kB/s device -> host; 64 B 2400-2500 round
  trips/s, 540 kB/s each way. Tested 15 Oct 2026 on B153A. Open: back to
  crates.io embassy once the next embassy-stm32 release ships;
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
- ✅ **FW-38 HardFault from flash prefetch across the banks** (ES0548 2.2.10): with prefetch on (FW-24) the CPU
  ran wrong instructions after a call from bank 2 into bank 1 (0x0804005c -> `BufReader::read_le32` at 0x08039d18)
  and HardFaulted with garbage in r0-r3, within seconds under `ww usb-loopback` (19 Oct 2026, B153A, the embassy
  main build of FW-24; the 0.6 fork build has the same bug, its layout just didn't hit a failing branch). ES0548
  Rev 4: "In rare cases, the code prefetch may fail upon branching and function calls across flash memory banks,
  when the device operates in dual-bank mode", 512-Kbyte devices always dual-bank, not fixed in revisions A, Z/3,
  Y/4, workaround "None". Proof with one binary: prefetch cleared at run time through the probe ran 8 loopbacks
  clean, set at run time on a build without it faulted in 8 s at the same instruction. Fix: prefetch stays off
  (`main.rs`). Cost (release opt-level 3): 1 KB 311-313 round trips/s, 631 / 491 kB/s (prefetch on: 332, 655 /
  512); 64 B 2.1-2.2k/s, 362-367 / 518 kB/s (2.3-2.4k, 441 / 536). Prefetch could come back with all code in bank 1:
  `.text` is 358 KiB at opt-level 3, 254 KiB at "s" (ends 0x0803f7e4, 2 KiB below bank 2), 240 KiB at "z".
: the firmware runs the same with or without a probe: `ww_defmt_logger`'s RTT tee
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
- 🐛 **FW-40 Small-message throughput down with event-driven observe** (WireWeaver SRV-25, branch server-size):
  `ww usb-loopback` 64 B host -> device 361-366 kB/s before SRV-25 and on 0.20.6, 301-303 kB/s with it (-17%); 1 KB
  host -> device 628-642 -> 618 kB/s; round trips and device -> host unchanged (B153A, 19 Oct 2026, same host and
  `ww`). Mostly fixed in WireWeaver: the generated `ObserveState::next_wake` returns at once while nothing is
  observed (336-339 kB/s), and `ObserveTable::is_active` is a flag instead of a walk over the slots: 343 kB/s at
  64 B, 621-628 kB/s at 1 KB (in the 0.20.6 range). Still ~5% short at 64 B: what is left is the event-driven loop
  itself (`select4` with `wait_until` and `OBSERVE.wait()`, a timer and two waker registrations per wake, critical
  sections on the M0); not looked into further.
- ✅ **FW-41 Hub bring-up loop after a probe-rs download and reset**: after `probe-rs download` + `probe-rs reset`
  (J-Link, B153A, 19-20 Oct 2026) the hub was reset every 0.6 s and Donguru never enumerated; `cnt run` came up
  at once. Two causes. (1) Not the firmware: probe-rs 0.32's STM32 ARMv6 `debug_device_unlock` sets DBGEN with a
  read-modify-write of RCC_APBENR1 over SWD while the core already runs, and puts back the value it read: when the
  firmware enables USB and SPI2 in between, both clocks go off again (APBENR1 `0x18410504` instead of
  `0x18416504`, USB registers all zero, no D+ pull-up). Hit about one boot in three after a probe-rs reset;
  `cnt run` halts the core first, a power-up without a debugger never runs it. Upstream notes: tpm
  `prj/P2534-donguru/design/upstream-probe-rs-g0-dbgen.md`. (2) Ours: FW-18's I2C probe counted the hub's NAK
  as a loss, and a hub with nothing on port 1 NAKs once the host talks to it, so it reset the hub forever. Now the
  probe reads VID/PID and only all-zero counts (FW-18); a boot hit by (1) stays off USB until the next reset
  instead of looping.
- 🚧 **FW-42 UART RX compression**: `uart[1].rx` is a WireWeaver compressed counted stream (API-55,
  `ww_stream::codec::Compressed` around `ww_lz4::Encoder<4096, 12288>`, h12 with 12 KiB history and 4 KiB kept,
  set up in place, encoder in RAM through ww_lz4's `ram`; `fw/donguru/src/ww/uart.rs` `RxState`): hosts that offer
  LZ4 get LZ4 blocks, older hosts (no offer) the chunks as before. The receive path behind it: `rx_task` reads the
  DMA ring (8 KiB) straight into a queue of 32 chunk slots (`src/uart.rs` `RxSlots`, replaces the 16 KiB pipe),
  each slot carrying the bytes lost before it so losses reach the host at the right position; after a read that
  didn't fill a chunk it waits 1 ms (`RX_GATHER`), else a busy line gave 1-byte reads and events; the server task
  sends at most 2 chunks per turn. Bench source: `debug.uart_replay` (feature `uart-replay`, 96 KiB of captures in
  `replay/`) loops a boot log, `ls --color` or `top` out of IO7 by DMA; `donguru-core` example `uart_rx_bench`
  checks every byte against the capture and reports kB/s, USB kB/s, ratio, CPU (FW-43) and losses. Tested 21 Oct
  2026 on B153A (jumper IO7-IO8), 5 s per run, every byte delivered correct: at 5.818 Mbaud (581.8 kB/s) raw
  505 kB/s (the rest lost, counted), LZ4 boot 482 kB/s at 2.8x, ls 551 kB/s at 3.9x, top 568 kB/s at 4.3x with
  none lost; 2 Mbaud all delivered, CPU 69 % raw / 68-74 % LZ4 (receive path alone 23 %). Needs two fixes not in
  this repo yet: WireWeaver's `RefVec<u8>` slice write (wire_weaver branch s/20oct2026-uart-rx-lz4, `perf(shrink_wrap)`)
  and embassy's DMA ring copy without a division per byte (tpm `prj/P2534-donguru/design/upstream-embassy-dma-ringbuffer-copy.md`,
  not merged upstream): with upstream embassy the receive path alone takes the whole CPU at 5.8 Mbaud and 16 kB/s
  get through. Open: point `[patch.crates-io]` embassy-stm32 at a fork with the fix until upstream has it (the
  person's call); bytes lost in the DMA ring itself (`uart_rx_errors`, a ring overrun) are not counted into the
  stream (none seen with the slot queue). Design notes: tpm `prj/P2534-donguru/design/uart-compression/notes.md`.
- ✅ **FW-43 CPU idle counter**: the thread-mode executor runs by hand (`src/cpu.rs`, `main` is a `cortex_m_rt::entry`
  spawning the `boot` task) and counts the time in WFE with interrupts masked (SEVONPEND wakes it) in the counter
  `idle_us`; `executor_polls` counts the wake-ups. CPU load over a span = 1 - Δ`idle_us` / span, from
  `device.counters` read twice (`uart_rx_bench` does it). Interrupt handlers count as busy. Tested 20-21 Oct 2026
  on B153A: ~95 % idle at rest, ~2 500 polls/s under UART load.
- 🧪 **FW-44 Status LEDs on shared time**: the LED task (`src/led.rs`) aligns breath / blink to
  `crate::sync::SYNC`'s shared time (`bedrock_led::Indication::run_with`, DEV-14) while a host has steered it
  through `clock` (API-57); free-running otherwise, as before. `crate::sync::SYNC` is a `bedrock_sync::Sync`
  (DEV-13), the shared clock itself, disciplined by `clock.steer` (`src/ww/clock.rs`, `ClockState`). Part of
  P2528's LED-clock-sync design (design/clock-sync.md): Donguru, rePort and B135 breathing in phase on one host.
  No sync line and no oscillator backend here (HSI16 trim too coarse, decided 21 Oct 2026). Flashed and run on
  B153A (omarchy-m1, J-Link, 21 Oct 2026): boots clean, 227 s with no panic once another session's concurrent
  probe-rs/gdb use of the same board (PC13/RTC_CR pokes, P2534 bootloader test) stopped. `ww sync` end to end
  (host-synced `Locked`, the residual, LEDs in phase with rePort) not yet exercised: that same interference left
  the hub's USB enumeration wedged on the host side (`dmesg`: "unable to enumerate" on hub port 1 since 17:51),
  so `donguru_api` never answered over USB during this session; needs a replug or a host-side USB port reset on
  omarchy-m1 to retest, nothing pointing at this firmware's own code (no panic in 227 s of clean running, and the
  same reset/enumeration failures the board showed before the two LED/clock pieces landed on it today).
- ✅ **FW-45 Boot loop into the ROM bootloader**: after 3 boots in a row without a healthy mark the fourth goes to
  the ROM DFU on its own (as B135 BOOT-5), so a firmware that crashes early can still be updated over USB. Every
  boot counts in the FW-12 boot word (`donguru_board::boot::BootWord`, its doc has the bit layout; host tests);
  healthy is USB configured or 10 s up (`HEALTHY_AFTER_S`), so a dongle that only charges never trips it
  (`boot::healthy_task`). The rescue boot brings up the hub and nothing else, then resets into the ROM with the
  request (`main.rs`, `boot::rescue`): after a power cycle no firmware has configured the hub yet. The next firmware
  logs it and reports `device.dfu.node` `last_result` `Recovery` until a requested update or a power cycle. Not
  caught: a crash after USB is configured or after 10 s, a crash in the hub bring-up itself. Test image: feature
  `crash-loop-test`. Tested on B153A (omarchy-m1, 21 Oct 2026), all over USB, no probe: the test image written
  with `ww dfu stm32 flash` -> 0483:df11 behind the hub (same serial) about 1 s after the ROM's jump, stays there ->
  the good image back with `ww dfu stm32 flash --follow`, `ww dfu status` last update `Recovery` -> `just update`
  clears it to `Nothing`; 5 `reboot --into app` in a row stay in the firmware. Not tested: the rescue after a power
  cycle (hub not yet held, the rescue's own bring-up), the 10 s mark without a USB host. tpm P2534#21oct-1844. ^v1.0
- ✅ **FW-46 Hub kept across MCU resets**: the hub stays attached and configured through a firmware update, crash or
  reset (the FW-12 RTC_OUT1 hold on rev A, the RESET_N pull-up on rev B), so a DUT's USB link on a downstream port
  survives them; only the MCU's own device on port 1 re-enumerates. At boot, after anything but a power-on or
  brown-out reset and with the reset line still high (rev A: RTC_CR, `donguru_board::boot::rtc_runs_hub`), the
  firmware reads back every register block it writes plus the attach bit (`donguru_board::hub::check`, host tests)
  and keeps the hub when all match; another firmware's config or a lost one gets the reset pulse and the config as
  after a power cycle. All-zero IDs are asked again for 1 s (HUB-5). A hub that does not answer at all is one the
  host suspended (Linux suspends a hub at once when no device below it is active, the MCU's own one gone with the
  reset; seen after a probe flash): kept, and `watchdog_task` reads it back once the MCU's USB has woken it, resets
  it if it is not this config or still silent after 5 s. Counters `hub_kept`, `hub_kept_reconfigured`. Code:
  `fw/donguru/src/hub.rs` (`Hub::keep`, `bring_up`), the rescue boot (FW-45) goes through the same `bring_up`.
  Tested on B153A (omarchy-m1, 21 Oct 2026): the hub stayed the same host device (no disconnect in `journalctl -k`)
  through a probe flash (hub suspended, verified 1 s after boot), 3 `ww dfu reboot --into app` and a ROM update
  (`just update`, kept in 7 ms); a firmware with another hub config (power-on time 100 -> 50 ms) written by ROM update
  found `Differs { register: 0x10 }` and reconfigured, so did the way back. FW-18 still recovers (RTC_CR POL pulsed
  through the J-Link, 3 times). With a DUT on the plug (B129A, self-powered, embassy `usb_serial` echo flashed
  over Donguru's own SWD-host): 5 `ww dfu reboot --into app` in a row, its tty open and echoing every 100 ms the
  whole time, 446 of 446 echoes back, the DUT the same host device throughout. Seen once: in the first run, right
  after B129A had been flashed and enumerated, it dropped and re-enumerated 1 s after the first reboot; not again.
  A ROM update keeps the hub but not the DUT's link: the ROM drives the plug data switch off (FW-50). A bus-powered
  DUT loses power at every MCU reset (FW-49). tpm P2534#21oct-1844b.
- 🧪 **FW-47 Rev A workarounds behind the feature `rev_a`**: on by default while B153A is the only board; a rev B
  release firmware is built without it, the first boards keep it for testing and jigs (Roman, 21 Oct 2026). Gated:
  PA4 `HUB_I2C_EN` driven high for the hub's SDA pull-up (`src/hub.rs`; rev B: R301 to 3.3 V, PA4 free), the plug
  CC drive on PD0/PD2 (FW-17, `src/swd.rs`; rev B: `TODO(FW-17)`, the debug-accessory Rp moves to the plug CC, its
  level not decided) and `device.board` revision "A" (else "B"). Not gated, the same on rev B: the hub reset on
  PC13 (rev B's schematic has it there too), the hub port swap (FW-6), the variant strap (FW-22). Source:
  `hw/b153_typec_serial/doc/hw_b153a.md` §0, `change/change_b153b.md`. Both builds checked; the default one is
  the firmware as before.
- 🐛 **FW-48 Panic in `DefaultHandler` during probe-rs attaches**: once in 5 RTC_CR pokes over the J-Link (each a
  separate `probe-rs write`, the core stalled long enough for ADC DMA overruns), 130 ms after a FW-18 recovery an
  unhandled interrupt ran `DefaultHandler`, whose `cnt!` / `error!` panicked in defmt (`default_panic`, nested
  logging), and the firmware reset (B153A, 21 Oct 2026, FW-46 build). Which IRQ is unknown: the backup counter
  `unhandled_exceptions_total` never got written. Not seen without a debugger. To do: log the IRQ number without
  defmt in the handler (a backup register), find the unbound interrupt.
- 📋 **FW-49 Plug VBUS kept across MCU resets**: PWR_EN (PA6, U601) has the R601 pull-down, so plug VBUS goes off at
  every MCU reset and stays off through the ~43 s ROM update; `plug_vbus_at_boot` only turns it on again once the
  ADC reads. A bus-powered DUT on the plug therefore loses its link even though the hub keeps it (FW-46). Needs a
  hold that survives the reset (no backup-domain output on PA6; rev B schematic) and a decision on safety: VBUS left
  on into a ROM session nobody supervises (no guard running). tpm P2534#21oct-1844b.
- 🐛 **FW-50 The ROM bootloader cuts the DUT's USB data (rev A)**: DATA_CUT (PA9, U701 S) is also USART1_TX of
  the STM32 ROM bootloader, which opens all its interfaces at once: in the ROM DFU (FW-12) PA9 is AF1 and idles high
  (B153A, 21 Oct 2026, J-Link: GPIOA MODER 0xe82baaaf, AFRH 0x110, IDR bit 9 set), so U701 switches the plug data to
  the test points and a DUT drops off the hub ~1 s after the ROM starts, for the whole ~43 s update, and enumerates
  again once the firmware runs (the hub itself stays, FW-46). PA10 (SBU_TO_USB, U901 S) is USART1_RX there, in
  the same way. Nothing the firmware can do; for rev B: DATA_CUT (and SBU_TO_USB) on pins the ROM leaves alone
  (AN2606, STM32G0B1 bootloader pins), `hw/b153_typec_serial/change/change_b153b.md`.
- 📋 **FW-52 Hub in its default mode (rev B)**: rev B lets the hub boot by itself (`hw/b153_typec_serial/change/
  change_b153b.md`, "Hub boots by itself"): straight D+/D- lanes, SCL pull-up switched by PA4 with a pull-down, so the
  hub latches CFG_SEL = 00 (default configuration, same as our SMBus config but the port swap) whenever the MCU does
  not drive PA4, and the ROM DFU reaches the host with the firmware dead. Firmware: early in boot (pre-init) hold
  PC13 low, decide default mode or SMBus (PA4 high, then release RESET_N and configure as now); FW-6's port swap off
  on rev B; FW-46 keeps a hub in default mode that answers nothing on I2C instead of resetting it after 5 s (the
  firmware's own USB reaching the configured state is the proof it works); FW-18's I2C probe only in SMBus mode, a
  default-mode hub recovers from a brown-out by itself. Needs the board revision at runtime (FW-22 strap or an ID
  divider) to pick the rev A or rev B path.

## API (`API`)

- 🧪 **FW-51 Firmware builds against the sibling checkouts, WireWeaver TR-15**: main stopped building (24 Oct 2026):
  `bedrock_crash` came from embedded_bedrock's git (cargo read a templated `bedrock_template/Cargo.toml` there), and
  `bedrock_led` / `bedrock_sync` brought WireWeaver a second time from its git, two `ww_indication` /
  `shrink_wrap` (13 E0308 in `ww/leds.rs`, `ww/clock.rs`). Now `bedrock_crash` is a path dependency on
  `../embedded_bedrock` like the other bedrock crates, and `fw/donguru/Cargo.toml` patches WireWeaver's git source
  to the sibling checkout, as `bedrock_led`'s Cargo.toml asks. Built against wire_weaver 689b697a: TR-15 (random
  setup token echoed in link setup, framer session reset; USB without prefix or checksum) comes in through
  `ww_device` / `wire_weaver_usb_embassy` with no firmware code change, a wire break: this firmware needs a host
  (CLI, `ww`) with TR-15 and the other way round. Dev and release build, not run on a board yet. To check on
  B153A: `ww list` / `donguru info` connect with today's `ww` and CLI, a reconnect (CLI run twice, replug) gets no
  stale reply, `uart[1].rx` and `device.log` stream as before.
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
  notes 256 B, in flash since API-19), `device.counters` (`ww_counters`: RAM and BKP buffers,
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
  (FLTB PC14, two updates low) latch until the next mode write, which re-arms. Logic in `donguru_board::power` (host tests),
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
  🧪 Samples (19 Oct 2026, `donguru_api` 0.3.0): `meter.reading` is `ro+observe+batched` (WireWeaver API-39);
  observed in `Samples` mode it sends every ADC sequence (622 µs, 32x oversampled, not averaged over `window`) with
  its own device time, as compressed `Series` batches of 32 rows. The ADC task writes the rows into observe cells
  (a ring of 64 in a static, `MeterSamples` in `ww/meter.rs`), only while a host samples; times are back-dated from
  the DMA half transfer by one sequence each. +3.8 KiB RAM, +44 KiB flash (the `Series` codecs of three quantities
  of any number type; FLASH 397 of 504 KiB). `dg power watch --samples` asks a 16 kB/s budget (the device's
  default 2 kB/s carries about a third). Tested 19 Oct 2026 on B153A + B129A at 5.2 V: 8014 samples in 4.98 s
  (1608/s), none missed, times increasing, the largest step 0.79 ms (back-dating jitter), current 6..26 mA. The
  first run after another one left 18 rows from 88 s before: WireWeaver's `ObserveState::reset` kept the cells'
  rows, fixed upstream (wire_weaver s/19oct2026-uart-config-eq, test `reset_drops_waiting_rows`), not yet rechecked
  on the board. CPU load of the encoding not measured (no cycle counter on the M0+).
- 📋 **API-13 Button and switch**: observable button (with a press counter) and slide switch, counted event stream.
  With FW-4. ^revB
- 📋 **API-14 SBU owner and UART on SBU**: one function at a time on the plug SBU lines, `sbu.release`; `uart[0]` as
  `ww_uart::Uart` with the TX/RX flip from `plug.orientation` and a two-line monitor. With FW-7. ^revB
  ✅ Header UART `uart[1]` (0.20.10, USART5 on IO7 TX / IO8 RX, `fw/donguru/src/uart.rs`, `ww/uart.rs`): `tx` credit
  sink, counted `rx`, `capabilities`, and `configure` (8N1 at 921 600 Bd from boot): 4 Bd ..= 8 Mbaud, 7 or 8 data bits,
  none / even / odd parity, 1 or 2 stop bits, `Asynchronous` only, 3.3 V; anything else `Unsupported`. Answers the rate
  the divider gives (embassy-stm32's choice, read back from BRR / PRESC / OVER8 after setting, `uart_rate_mismatches`
  counts a difference): PCLK 64 MHz / k, so above 4 Mbaud only 8, 7.11, 6.4, 5.82, 5.33 Mbaud (OVER8 drops USARTDIV's
  lowest bit, RM0444); 6 Mbaud gives 5.818 Mbaud (-3.0 %, at the edge of what a crystal-clocked DUT tolerates; exact
  6 Mbaud needs a 60 or 48 MHz PCLK). `idle_chars` answers 1 (the IDLE flag), chunks to the host are not cut on idle.
  A new line waits for the TX DMA write in flight; bytes in the RX ring are dropped. RX ring 8 KiB, 32 RX chunk
  slots (16 KiB), `rx` compressed for hosts that offer LZ4 (FW-42).
  `uart[1].set_pin_level`, `send_break`, `tx_pending` still `Unimplemented`. Tested 20 Oct 2026 on B153A (jumper IO7-IO8):
  `dg uart configure` answers 927 536 / 3 047 619 / 5 818 182 / 8 000 000 Bd for 921 600 / 3M / 6M / 8M, 115 108 7E2,
  `Unsupported` (exit 4) for 9M and 9N1; `uart_resume --mode plain --baud` loops 2 MB back exactly, in order, none
  lost, at all three rates: 70 kB/s at 921 600, 111 kB/s at 3M, 114 kB/s at 6M (host -> device limit, API-56); counters
  `uart_rx_errors`, `uart_rate_mismatches`, `uart_config_errors`, `uart_tx_overruns` 0. RX saw back-to-back bursts of up
  to 2 KiB at 5.8 Mbaud. Sustained 580 kB/s on RX: FW-42.
- 📋 **API-56 UART host -> device rate**: the header UART's `tx` carries ~120 kB/s at any baud rate (B153A, 20 Oct 2026,
  2 or 8 KiB credit alike): one `tx` item of at most 256 B (`ww_uart::UART_TX_MAX`) per message, ~470 messages/s over
  USB. Several items per USB transfer (WireWeaver) or bigger items would lift it towards the ~620 kB/s USB carries.
  Enough for typed commands; a fast DUT input path needs it.
- 📋 **API-15 GPIO banks**: plug (SBU1, SBU2, A2, B2) and header (IO7..IO10) as `ww_gpio::Bank` with pin events.
  With FW-10. ^v1.0
- 📋 **API-16 I2C buses**: plug SBU (I2C1) and header (I2C3) as `ww_i2c::I2c`. With FW-10. ^v1.0
- 📋 **API-17 Hub and SWD state**: observable hub state and recoveries, `hub.recover`, observable SWD session; FW-18's
  spurious recoveries after a cold boot fixed. ^revB
- 🧪 **API-18 Bring-up API**: MCU pins by port and number (refused list, no memory write), boot-time pin snapshot,
  hub SMBus registers, UCPD1/2 state and role, raw LEDs, peripheral register reads (P2534#14oct-1014). ^revB
  Done: rules in `donguru_board::pins` (refused list, the pin claim table `Claims` with its holders: LEDs, meter,
  button, header UART, V_SBU select from boot, an SWD session and its plug CC drive, a UCPD role; the electrical
  rules: tied nets, SBU_OE with PA0 / PA1, PA0 / PA1 push-pull without V_SBU, `~PWR_FLT` push-pull; reg_read's
  peripheral blocks) and `donguru_board::ucpd` (roles, Rp refused on UCPD1 of rev A), unit-tested; firmware
  `fw/donguru/src/pins.rs` (boot capture first thing in `main`), `ucpd.rs`, `hub.rs` (SMBus block read / write
  through the watchdog task, STATUS/COMMAND refused), `ww/debug.rs`, behind `debug-api` (D2); the mock in
  `donguru-core` runs the same rules. Check on B153A: every allowed pin set and read back (`dg debug pin set
  RELAY_EN high`: the relay clicks), `dg debug pins --boot --diff` clean against verify_b153a.md §0, hub VID/PID
  via `dg debug hub read 0x00 4`, `dg debug ucpd` reads both ports and `dg debug ucpd 2 --role rd` / `off`, an SWD
  session takes its pins back (`InUse` while connected), `dg debug reg 0x50000000 10` matches `dg debug pins`.

- 🧪 **API-19 Persistent settings**: `device.settings` and `device.label` kept in flash by WireWeaver's `ww_settings`
  store (STD-32: a sparse record of the fields not at their default, power-cut safe, readable by older and newer
  firmware) in the last 4 pages of bank 2 (`SETTINGS` in `memory.x`, 8 KiB, was the template's 2 KiB CONFIG page),
  read through `ww_ecc_flash` (SRV-19: a word cut by a power loss reads as zeros, not an NMI on every boot). Fields:
  label name and notes, `plug_vbus_at_boot` (applied through the guard once the ADC has a reading, refused above its
  `max_voltage`: VBUS stays off, `plug_vbus_at_boot_refused` counted), `meter_window` / `meter_deadband` (applied
  live and at boot), `button_action` and `uart_plug` (stored only until FW-4 / API-14 use them); `reset` puts every
  field and the label back to the default. Every write answers once the record is in flash (`Fault` code 1 flash,
  2 a newer firmware's record kept until `reset`, 3 record too big); the page erase ahead runs in the server task
  after the answer (~25 ms). Code: `fw/donguru/src/settings.rs`, handlers in `ww/device.rs`. +2.3 KiB RAM, +38 KiB
  flash (half of it the served properties' serdes, linked now that they answer; FLASH 351 of 504 KiB). The label
  at boot also goes into the WireWeaver interface's USB string descriptor (`ww list` LABEL column, a new label
  shows after the next boot). Factory key (FW-9 calibration) not used yet. CLI: `dg settings` (CLI-47).
  Tested 19 Oct 2026 on B153A + B129A (Python bindings, J-Link on omarchy-m1): a store left by an earlier bench test
  (label "bench-1") loaded as is; every setting and the label written (3-6 ms per write, the record programmed
  before the answer), out-of-range `meter_window` and a 0 V `plug_vbus_at_boot` refused with `InvalidArgument` and
  their reason; after a reset everything read back, the live deadband came from the setting, and
  `plug_vbus_at_boot` = ForceOn 6 V switched plug VBUS on at boot through the guard (5.225 V, 16 mA). Not tested:
  `reset`, a power cut during a write, the `ww list` label (built after the board test). ^revB

- 🧪 **API-57 `clock`, `ww_timebase::Clock`** (`donguru_api` 0.5.0): `now()` / `info()` from the embassy-time driver
  (TIM4, 1 µs resolution); the whole `host_sync` group served (`steer`, `shared_now`, `set_sources`, `sources`)
  through embedded_bedrock's `bedrock_sync` (DEV-13, FW-44), so a host (`ww sync`) steers Donguru's shared clock
  onto UTC and the status LEDs align to it. No sync line on this board: `configure_sync` always `Unsupported`,
  `set_time_of_day` always `WrongState`, `info().sync_formats` empty. Code: `fw/donguru/src/ww/clock.rs`
  (`ClockState`), `fw/donguru/src/sync.rs` (the static `SYNC`). Tested against the mock and `just test`/`just
  lint`; flashed and run on B153A 21 Oct 2026 (FW-44 has the board test and what blocked `ww sync` there).
- ✅ **API-58 `device.boot` and `device.dfu.node`** (`donguru_api` 0.5.1, `ww_dfu`): `BootControl` with
  `reboot(App / Updater)` (the reply goes out, the reset 50 ms later), `boot_state` always `Confirmed`, `confirm`
  `Ok`, `revert` `Unsupported`; `node` says `Profile::InPlace`, unsealed, `boot_control` only. `Updater` is the
  STM32 ROM bootloader (FW-12). Code: `fw/donguru/src/ww/boot.rs`. `ww dfu reboot` tested on B153A 21 Oct 2026.

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
  case, unique prefix), USB path (`5-1.2`) or user label (`label:L`, or plain text no serial matches; any case),
  auto-select when exactly one; `Dongle::open` with the version check.
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
- 🧪 **CORE-4 Python bindings** (`donguru-py/`, PyO3 + maturin, package `donguru`, abi3 wheel for CPython ≥ 3.10):
  blocking API with SI floats, enums as strings, one exception class per error category (`donguru.Error`,
  `NoDevice`, `DeviceError` with `.kind`/`.item`, `Unimplemented`, `Timeout`, `Incompatible`, `LinkError`).
  `donguru.list()`, `donguru.open(selector)` (index, serial, path, label), `Dongle` with `info()`,
  `firmware_version`, `label`, and the levels `power` (on/off/cycle, trip), `meter` (read, `watch()` iterator:
  periodic, on change, samples; totals, temperature, window, deadband), `usb`, `header` (relay, VBUS_SW),
  `uart.plug`/`uart.header` (`configure`), `settings`, `leds.identify`. `donguru.mock.MockDevice` (CORE-3's mock)
  ships in the wheel for script tests; type stub `_donguru.pyi`. `just py-test` (pytest against the mock, also in
  `just test-all`), `just py-build`; docs `docs/user/python.md`. Not run against a dongle yet; on B153A check:
  `donguru.list()` and `open("label")`, `power.on(trip=...)`/`off`/`cycle` with a DUT, `meter.watch()` at 0.5 s,
  `on_change` and `samples` (rate, `t`, Ctrl-C), `uart.plug.configure(baud=...)` answer, `settings` round-trip over
  a reset. The rest of the labs' shape is CORE-13. ^v1.0
- ✅ **CORE-5 SWD link memtest** (`donguru-core/src/swd_memtest.rs`): memtest-style patterns over any
  `WordMemory` (data bus walking 1/0, Barr address bus, solid, checkerboard, address-in-address, moving inversions
  per chunk, byte lanes, seeded xorshift64* random), mismatches counted and located (address, expected, read,
  flipped bits). Unit-tested against fake RAM with a stuck bit, an aliased address bit, swapped byte lanes and rare
  read flips. Feature `probe-rs` (`swd_memtest/probe.rs`): attach through any CMSIS-DAP v2 probe at a given speed,
  halt, save the region, run, restore, resume; probe-rs without its C HID backend.
- ✅ **CORE-6 Mock's VBUS guard refuses like `donguru_board::power`**: writing a guarded mode while VBUS is over
  `max_voltage` answers `Refused` and changes nothing, on `plug.vbus_mode` and `header.vbus_sw_mode`
  (`donguru-core/src/mock/mod.rs` `MockState::apply`), as `Switch::set_mode` does and `docs/user/power.md` says.
  Tested in `donguru-core/tests/mock.rs` (`plug_vbus_guard_and_cycle`); a `Tripped` state for CLI tests is put into the
  mock state directly.
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
- 📋 **CORE-13 Python, the rest of the labs' shape** (after CORE-4): `dg.meter.record(seconds)` → pandas DataFrame
  (optional extra), `dg.inputs.events()` iterator (button, switch), `dg.usb.cycle(wait_for=...)`, `measure()`
  helpers, the GPIO / I2C / ADC levels as their API items land, an asyncio API beside the blocking one, wheels per
  platform on PyPI (`uv add donguru`). ^v1.0

## CLI (`CLI`)

- ✅ **CLI-1 Command tree and global flags**: `info`, `device`, `usb`, `gpio`, `udev`, `config`, `completions`;
  `--device`, `--format text|json|jsonl`, `--color`, `-v` / `-q` (`donguru-cli/src/cli.rs`).
- ✅ **CLI-2 Colour theme** adapted to terminal capabilities (`src/theme.rs`, `examples/theme_preview.rs`).
- ✅ **CLI-3 Shell completions** (`donguru completions <shell>`): since CLI-49 the scripts are dynamic.
- ✅ **CLI-4 External subcommands** (`donguru-cli/src/commands/external.rs`): `donguru NAME ARGS` runs `donguru-NAME` from
  `PATH` (exec on Unix, exit status passes through), with `DONGURU_DEVICE` and `DONGURU_OUTPUT` set from the resolved
  context (the design doc's names); an unknown name exits 2 with a "did you mean" hint over built-ins and plugins;
  `donguru help` lists the plugins found. Host-only, tested with fake plugins.
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
- 🧪 **CLI-8 `udev`** (`donguru-cli/src/commands/udev.rs`, logic in `donguru-core/src/udev.rs`): `generate` writes the rule
  (uaccess / group / all; the dongle `c0de:cafe` and the STM32 ROM bootloader `0483:df11` for `fw update`); `install`
  / `uninstall` write or remove the rules file (atomic, `--force`, `--dry-run`, reload with `udevadm`), and on
  permission denied re-run themselves through `pkexec` (one prompt) or print the exact `sudo` command; `status`
  reports the installed rules (also in the vendor dirs) and each connected dongle's node, mode, owner and whether
  this user can open it (exit 1 if not; text / json). Tested: core against tempdirs and a fake sysfs, `generate` and
  `status` run for real; `install` / `uninstall` were never run on a real `/etc` (needs root). On a Linux PC with a
  board check: `sudo dg udev install`, replug, `dg udev status` shows accessible without sudo; `pkexec` path from a
  desktop session; `uninstall` removes it; `fw update` reaches the DFU device without root. ^revB
- 🧪 **CLI-9 `config generate / show / path / validate`** (`donguru-core/src/config.rs`, `donguru-cli/src/commands/config.rs`,
  `context.rs`): layering defaults < `/etc/donguru/donguru.toml` < `$XDG_CONFIG_HOME/donguru/donguru.toml` < `./donguru.toml`
  (walking up) < `DONGURU_*` < flags, per key, for the keys `device`, `output`, `color`. `show [KEY] [--origin]` (json
  too), `path [SCOPE]`, `validate [PATH]` (unknown key or bad value: exit 1), `generate [-o FILE] [--force]` (all keys
  commented out). A broken file is skipped with a warning by the other commands. Open: more keys (timeouts, verbosity)
  once something reads them; the three-state merge of the design doc is not done (a layer cannot set a key back to None).
- ✅ **CLI-10 `fw version` / `fw update`** (`donguru-cli/src/commands/fw.rs`, `donguru-core/src/update.rs`): `fw version`
  prints name, version, git commit (`-dirty`), profile and the API version (`--format json` too). `fw update [ELF|-]`
  (default `fw/donguru/target/thumbv6m-none-eabi/release/donguru`) reads the old version, `device.boot.reboot(Updater)`
  (API-58), follows the dongle by its USB serial into the ROM bootloader with wire_weaver_client
  `follow::stm32_dfu_update_with`, erases, writes and verifies, waits for the firmware, reconnects by serial and prints
  old -> new; progress bar on stderr, json report. Logic in `donguru_core::update` (typed `FirmwareImage`,
  `Dongle::update_firmware`, `update_with` takes the bus, DFU interface and reconnect: mock + simulated ROM tests).
  A firmware with the 24-hex serial gets the ROM serial from its UID. Tested on B153A (omarchy-m1, 21 Oct 2026):
  three updates in a row, 44.1-44.7 s each, version read back. Docs: `docs/user/firmware-update.md`. Not done: a
  firmware that does not start (FW-45), `--wait`/timeouts flags. tpm P2534#21oct-1844c. ^v1.0
- ✅ **CLI-11 `power status / on / off / cycle / meter / watch / totals`** (`donguru-cli/src/commands/power.rs`):
  `status` tables plug + header + meter in one read; `on` (`--allow-voltage`, `--trip`) and `off` read back `vbus`
  after the write and fail the exit code unless it ended up where asked (`On` / `Off`), covering a trip or a latched
  fault without the host needing its own copy of the guard; `cycle --off --release-sbu`; `meter` one reading;
  `totals [--reset]` in mAh / mWh; `watch [--interval | --on-change | --samples]` streams `meter.reading` through
  CLT-28 observe until Ctrl-C (`--format json` is refused: use text or jsonl); `--samples` (API-39, 19 Oct 2026) prints
  every ADC sequence with its device time, `--interval` then the batch age, missed samples on stderr (mock-tested
  in `donguru-core/tests/mock.rs` `meter_reading_samples`, not yet on a board). `donguru_core::vbus` builds the mode, and `cycle`'s
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
- ✅ **CLI-13 Exit codes settled** (`donguru-cli/src/exit.rs`, `ExitCode::code` / `name`, a test pins them): 0
  success, 1 failure, 2 bad args, 3 no device, 4 device error, 5 timeout, as `docs/user` already promised;
  `developer-notes.md` and `docs/design/cli/index.md` ("Exit codes") follow it. A broken link is 1; a transport code
  would be 6 if scripts need it.
- ✅ **CLI-14 `--version` with git SHA and build time** (`donguru-cli/build.rs`, no extra crates): `donguru 0.20.20
  (554cf91-dirty, built 21 Oct 2026 18:53 UTC)`; `SOURCE_DATE_EPOCH` honoured, `unknown` without git. Shows `donguru`
  also when run as `dg`.
- ✅ **CLI-15 Output renderer** (`donguru-cli/src/output.rs`): commands build a typed value that implements
  `Render` (text with the theme, json, jsonl records) and hand it to `Output`, which writes the selected format;
  streams use `Output::record`. Every command that prints a result goes through it: `power`, `header`, `usb`, `uart`,
  `info`, `device`, `settings`, `fw`, `swd memtest`. json shapes and plain text are unchanged (the insta snapshots
  from before pass as they were; new ones for `settings`, `fw`, `swd memtest`, `device`).
- 🧪 **CLI-17 `swd orientation / release`** (`donguru-cli/src/commands/swd.rs`): `swd orientation [auto|normal|swapped]`
  shows or sets `plug.orientation` and reads it back (exit 1 if it didn't stick); `swd release` calls `sbu.release`
  and reads `sbu.function` back (exit 1 unless `off`); text / json. An item the firmware lacks exits 4 with "the
  device's firmware does not support this yet". Tested against the mock only (`swd::tests`, `exit::tests`). On B153A
  check: `dg swd orientation` prints `auto`; `swapped` then `normal` read back; with a probe-rs session running
  `dg swd release` ends it and prints `sbu: off`; same on a firmware whose `sbu.release` is `Unimplemented` (exit 4,
  plain message). CMSIS-DAP itself (FW-15) is not a `donguru` subcommand, probe-rs talks to it directly over USB. ^revB
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
- 🚧 **CLI-20 `sbu` and `uart`**: `sbu status / release / orientation`, `uart open / monitor / pty / break` (API-14). ^revB
  Done: `uart configure BAUD [8N1] [--port header|plug]` prints the line set and its error from the asked rate
  (`donguru-cli/src/commands/uart.rs`); the setting lasts until the dongle resets.
- 🧪 **CLI-21 `led mode / brightness / identify`** (`donguru-cli/src/commands/led.rs`, API-9): `led mode
  [all-off|essential|normal]`, `led brightness [lowest..highest]` show or set and read back (exit 1 if it didn't
  stick), `led identify [SECONDS]` (default 5, at most 600; sent as whole ms, `Leds::identify_secs` in core). Mock-tested
  (`led::tests`). On B153A check: the three modes visibly change the LEDs and read back; brightness steps are visible
  and `lowest` is dim; `identify 3` blinks both LEDs for 3 s and returns to the mode; a second `identify` during one
  exits 4 (`Busy`); on a firmware without `leds.brightness` / `leds.identify` exit 4 with the plain message. ^revB
- 📋 **CLI-22 `adc`, `input`, `hub`**: `adc read`, `input status / watch`, `hub status / recover` (API-12, API-13,
  API-17).
- 🧪 **CLI-23 `debug`**: pins by net name or port pin, boot-state diff, hub registers, UCPD, register reads (API-18).
  ^revB Done: `dg debug pin get|set|af PIN`, `pins [--boot] [--diff]`, `hub read|write`, `ucpd [1|2] [--role]`,
  `reg ADDR [N]` (`donguru-cli/src/commands/debug.rs`, net-name completion), decode and reset diff in
  `donguru_core::debug`, insta snapshots against the mock; refusals exit 4 with the firmware's reason. Docs
  `docs/user/debug.md`. Not run against a board yet (API-18's check list).
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
- ✅ **CLI-47 `dg settings show / set KEY VALUE / reset`** (`donguru-cli/src/commands/settings.rs`, API-19):
  `show` prints every setting (text/json/jsonl); `set` changes one: `plug_vbus_at_boot off|on [--max-voltage
  V]|auto --trip A [--max-voltage V]` (reuses `donguru_core::vbus`, as `dg power on`), `button_action
  off|toggle-vbus|cycle-vbus|toggle-data`, `meter_window DURATION` (0.9 ms..=14.9 ms, one ADC DMA half buffer, FW-3; e.g. `10ms`), `meter_deadband
  VOLT,AMPERE` (e.g. `20mV,10mA`) or `--voltage`/`--current`, `uart_plug "BAUD BITSPARITYSTOP"` (e.g. `"115200
  8N1"`, read-modify-write: only baud/bits/parity/stop change, the rest of `UartConfig` stays); `reset` calls
  `settings.reset` with no confirmation, printing what it resets first (every setting and the label; factory data
  stays). `KEY` is a `ValueEnum` (snake_case, one line of help each) for live shell completion (CLI quality rule).
  A write's `CommonError::Fault` (the flash write failed / a newer firmware's record / a record too big) reads as
  a sentence (`donguru_core::Error::fault_note`, CORE-1), not just "Hardware fault 0x1"; the mock's
  `settings_fault` simulates all three for tests. The design doc's `reset --factory` isn't in the `Settings` API
  (API-19) and so isn't a flag here. Tested against the mock (`donguru-core/tests/mock.rs`'s `settings` test,
  `donguru-cli/src/commands/settings.rs`'s unit/insta tests); not yet on hardware (API-19's flash store is
  built in session `ww-latest`).
 Tested 19 Oct 2026 on B153A: show, set of every key (a too-long `meter_window` refused with the device's message), reset (label and settings to defaults).
- ✅ **CLI-48 `Broken pipe` when stdout closes early** (fixed 21 Oct 2026, `exit::is_broken_pipe`: ends quietly with exit 0; checked on B153A with `dg power status | head -1`). Was: `dg power status | head -2` prints `donguru: error: Broken pipe (os error 32)` on stderr; a closed stdout should end the command quietly (exit 0 or the usual SIGPIPE status), as other CLIs do.
- ✅ **CLI-49 Live shell completion** (`donguru-cli/src/completion.rs`, clap_complete `unstable-dynamic`): bash, zsh
  and fish (and elvish, powershell) call back into the binary. `--device` and `device select` offer the connected
  dongles (serial and USB path, label as help, 2 s limit), `fw update` / `--elf` ELF files, `uart configure` baud rates
  and framings, the rest comes from clap with the doc comments as help (commands, flags, setting names, enum values).
  `dg completions --install [SHELL]` writes `donguru` and `dg` into the per-user directories, `just install` calls it.
  Not done: `settings set KEY VALUE` values by key, `swd memtest --chip` (probe-rs chip names) and `--probe`, gpio pin
  names once CLI-5 is real. Checked in fish and by hand on omarchy-m1; bash and zsh scripts generated, not run.
- ✅ **CLI-50 `just install` / `just deploy`** (`justfile`): `cargo install --path donguru-cli --locked`, `dg` as a link
  to the same executable in the cargo bin directory (not a second `[[bin]]`), completions; `deploy` refuses on local
  changes, pulls `--ff-only`, `deps-pull`, installs and checks `--version` against HEAD.
- ✅ **CLI-51 Colour in text output** (`donguru-cli/src/theme.rs`, `Theme`: key, heading, ok, warn, bad, value,
  dim; `Painted` pads the visible text): every text renderer takes the theme, coloured per `--color`, `NO_COLOR`,
  `TERM=dumb` and whether stdout is a terminal, 24-bit or 16 colours per the terminal; json / jsonl stay plain.
  `swd memtest`'s private `Paint` is gone. Checked in the snapshot tests (plain) and by the unit test of `Painted`;
  the colours themselves on a terminal not looked at yet.
- ✅ **CLI-52 Errors under `--format json`** (`donguru-cli/src/output.rs`, `report_error` / `error_json`): text keeps
  `donguru: error: ...`; json and jsonl write `{"error":{"exit","code","message","kind?"}}` as one line on stderr
  (stdout stays data), `exit` the exit code's name (CLI-13), `kind` the dongle's error kind. Documented in
  `docs/user/scripting.md` and `docs/design/cli/index.md` ("Errors under json"). Unit-tested; no hardware involved.
- 🧪 **CLI-53 Progress for the other slow commands** (`output::Spinner`, on stderr, hidden with `-q` or when stderr
  is not a terminal; `Output::with_spinner` clears it around each record and counts them): `power cycle --off` of
  1 s or more counts down ("plug VBUS off, back on in 3.2 s"), `device log --follow` ("following the device log, 12
  lines"), `device counters --watch` ("reading the counters every 1000 ms, 3 snapshots"); `swd memtest` uses the same
  spinner. Built and unit-tested, not run against a board: on B153A check `dg power cycle --off 5s` (countdown, then
  the usual `plug vbus: on` line, nothing left on the terminal), `dg device log --follow` while pressing the button
  (lines print above the spinner, the count goes up), `dg device counters --watch -f jsonl` (spinner on stderr, clean
  jsonl when piped: `| cat` shows no spinner), `dg swd memtest` (spinner per pattern as before).
- 🐛 **CLI-54 Argument checks inside commands exit 1, not 2**: clap's usage errors exit 2 (`bad_args`), but a
  value a command checks itself (`settings set ... auto` without `--trip`, `power watch -f json`, a bad `uart_plug`
  value) bails with a plain anyhow error and exits 1 (`failure`). A typed usage error mapped to `ExitCode::BadArgs`
  would fix it; it changes those exit codes, so it needs a CHANGELOG entry. `docs/user/scripting.md` says so for now.

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
- 🧪 **PKG-3 Packages**: `.deb` / `.rpm` first, then Homebrew and Chocolatey; man pages, completions and the udev rule
  installed by the package (`docs/design/packaging/`, `docs/user/installation.md`). `just package` builds both:
  `xtask` (man pages via `clap_mangen`, completions via `donguru_cli::completion` directly, no shelling out to the
  built binary) feeds `cargo-deb` / `cargo-generate-rpm`, driven by `[package.metadata.deb]` /
  `[package.metadata.generate-rpm]` in `donguru-cli/Cargo.toml`; `packaging/` holds the udev rule and deb
  maintainer scripts (rpm scriptlets are inline in Cargo.toml). CI builds both as `upload-artifact`s
  (`.github/workflows/ci.yml` `package` job), no publish step. Verified by content: `dpkg-deb -c`/`-I` and
  `rpm -qlpv`/`--scripts` (via rootless `podman` containers, no `dpkg-deb`/`rpm` on this box) confirm both packages
  carry the binary at 755, every other asset at 644, the `dg` symlink and udev reload in postinst/postrm (rpm:
  inline scriptlets), and `License: TBD` from the workspace. Not yet done: an actual `apt`/`dnf` install on a real
  machine (needs root, done deliberately only as a content check tonight), `dpkg-shlibdeps`'s `Depends: $auto`
  (neither this box nor the minimal `debian:stable` container used to check it had `dpkg-dev` installed; CI's
  `package` job installs `dpkg-dev` so it should resolve there — check the next CI run). ^v1.0
- 🚧 **PKG-5 Lab book**: 48 labs in six levels (first hour, bench, automation, developer, hacker, insane), one Typst
  file each in `labs/` (goal, setup, steps in CLI / Python / GUI, what you should see, what next), built into one
  PDF (`just labs-book`) and HTML pages with per-lab PDFs (`just build-labs`, Typst 0.15 bundle export,
  experimental). Labs describe Donguru as it should be: each lists the feature IDs it relies on, the header shows
  their status read from FEATURES.md, and the book ends with a feature map, missing first. Most features they use
  are 💡 items added for them. Missing: photos, real output from a rev B board, publishing (docs CI needs Typst and
  the vhrd package, P2534#17oct-1150). Catalog and findings: tpm `prj/P2534-donguru/design/lab-book*.md`.
- 📋 **PKG-4 License**: the workspace says `license = "TBD"`. Still genuinely undecided (Roman's call, not
  written on his behalf), but now wired through consistently: `donguru-cli` and `donguru-core` were missing
  `license.workspace = true` (every other crate had it), so both PKG-3's packages read `TBD` straight from the
  workspace with no separate copy anywhere. No `LICENSE` file: cargo-deb's auto-generated `copyright` just says
  "Copyright information missing" with `License: TBD`, which is honest rather than a text claiming terms that
  don't exist yet. Decision recorded in tpm `prj/P2534-donguru/design/night-24oct-decisions.md` ("pkg"). ^v1.0
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
