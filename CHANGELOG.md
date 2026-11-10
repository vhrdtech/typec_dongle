# Changelog

All notable changes to Donguru: firmware, API crate, host library and CLI. The format follows
[Keep a Changelog](https://keepachangelog.com/), versions follow [Semantic Versioning](https://semver.org/). Feature
IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

### Changed

- Tested on B153A, Linux: a host bus reset keeps the hub (FW-68, `bedrock-usb-ch9` 20/20), and a hub reset still recovers within 1 s.

## [0.20.49] - 2026-11-10

### Fixed

- Firmware: a USB bus reset from the host (bedrock-usb-ch9 bus-reset, `usbreset`, a driver's error recovery) no longer drops the whole hub and every downstream port (FW-68). `UsbControl::reset` asked the hub watchdog to recover on any bus reset after the first configuration, and the watchdog reset U301 unconditionally; it now reads the hub back first (`Hub::check`, FW-46) and recovers only when that confirms a real loss, counting a host-only reset instead (`cnt!(usb_reset_hub_kept)`).

## [0.20.48] - 2026-11-09

### Changed

- Tested on B153A, Linux: USB on bedrock_usb (FW-66) enumerates, `ww usb-loopback` runs as fast as embassy-usb with 0 lost, `bedrock-usb-ch9` 19 of 20 pass (embassy-usb 17). Both drop off the bus on a USB bus reset (FW-68).

## [0.20.47] - 2026-11-09

### Added

- Firmware: the CMSIS-DAP v2 function has its own MS OS 2.0 function subset (WinUSB, CMSIS-DAP v2 interface GUID), so Windows binds WinUSB to it without a driver install (FW-66).

### Changed

- Firmware: USB on embedded_bedrock's `bedrock_usb` (FW-66): the composite device (WireWeaver EP1/EP2, CMSIS-DAP v2 EP3) is a `const`, served by bedrock_usb's usb_v4 driver, EP0 control machine and WireWeaver class; embassy-usb, embassy-stm32's USB driver and `wire_weaver_usb_embassy` are gone from `fw/donguru`. Same descriptors and endpoints as before, except descriptor 0x40 naming the stack `bdus` (was `wwue`). Release builds against main 5979815, embedded_bedrock 9f1a2c6 and wire_weaver e86f257 (`llvm-size`, text / data / bss in bytes; the text change also holds the newer WireWeaver): opt-level z default features 254,460 / 724 / 61,280 → 244,540 / 844 / 60,208 (text −9,920, RAM −952); z with `swd-host` 264,812 / 724 / 62,720 → 254,908 / 868 / 61,688 (text −9,904, RAM −888); opt-level 3 default 377,232 → 365,920 text (−11,312, RAM −952); 3 with `swd-host` 393,048 → 383,104 (−9,944, RAM −872). USB code from DWARF (`bedrock-lint code-size`: embassy-usb, embassy-stm32 `usb/`, wire_weaver_usb_embassy and `cmsis_dap.rs` before, bedrock_usb and `cmsis_dap.rs` after): z 11,156 → 8,488 B, z `swd-host` 13,114 → 10,176 B, 3 11,866 → 9,466 B, 3 `swd-host` 13,554 → 12,666 B. Stack (`bedrock-lint frames`, deepest static path, `swd-host`, z / 3): `usb_task` 1,548 → 1,372 / 1,116 → 1,044 B, `dap_task` 1,644 → 1,492 / 1,220 → 1,172 B, `server_task` 4,924 → 4,924 / 12,852 → 4,388 B, boot task 11,116 → 11,124 / 10,460 → 10,468 B. Not run on a board.
- `donguru_api` 0.7.0, wire version `w1.0` (API-64): WireWeaver e86f257 and exact_units 5c960fd, firmware and hosts update together (`Hello` in link setup, a wire break; old CLIs don't connect to the new firmware or the other way round). `meter.temperature` is a point on the Celsius scale in 0.001 K steps (`DieTemperature`), its number unchanged. The `adc` temperature channel reports kelvins (was °C): 233.15 to 398.15 K, as a script reading `adc` sees. `#[since]` markers dropped, all items are in `w1.0`. Snapshots of 0.5.2 and 0.6.0 removed (the checker's new format can't read them).
- `donguru_api` 0.7.0, raw-unit fields become `exact_units` types (API-65), the same wire break as API-64: `MeterCalibration.vbus_in_gain` and `current_gain` (were `vbus_in_gain_ppm`, `current_gain_ppm`; `Fixed<Ratio, Micro, i32>`), `LedMode::Blink { on, off }` and `Breath { period }` (were `on_ms`, `off_ms`, `period_ms`; `Millis<u16>`), `debug.hub_read`'s `len` (`Bytes<u8>`) and `debug.uart_replay`'s answer (`Bytes<u32>`). The numbers are the same. `donguru` CLI output and flags don't change, these fields aren't printed by it; `ww` and introspection show the new names and units. Release text at opt-level z 244,540 → 245,536 B with exact_units fb1683a taken since the FW-66 numbers, RAM unchanged (844 + 60,208 B). `ww_timebase::SteerPoint` and the `ww_i2c` slave fields keep their names: they are WireWeaver's.

### Removed

- `fw/donguru/examples/usb_serial.rs`, the embassy-usb CDC ACM echo example: bedrock_usb has no CDC class and the firmware talks WireWeaver (FW-66).

## [0.20.46] - 2026-11-08

### Changed

- Firmware: embassy from the romixlab/embassy fork at its default branch `main` (1ab8deec, plain git dependency, Cargo.lock holds the rev) instead of its `donguru-ring-copy` branch (FW-42). `main` is now the one integration branch for all our firmware: upstream 262fdba with each fix merged in from its own `t/<slug>` branch. For Donguru it carries the same DMA ring copy as before, plus `t/tim-psc-round` (the TIM prescaler rounded to the nearest tick, the same divisor at 64 MHz) and `t/h7-fracn` (H7 only). Release FLASH .text 246 720 -> 246 728 B, RAM unchanged; dev and release build, `just fw-lint` clean. Not run on a board yet.

## [0.20.45] - 2026-11-07

- docs: internal references removed

## [0.20.44] - 2026-11-04

### Changed

- wire_weaver STD-102 (`ww_si` on the `exact_units` integer kernel, no floats on the device): the firmware's UART baud rate and IO level, the mock UART and the PD log time read quantities with `to_in::<u32>(prefix, Truncate, Checked)` instead of `to_u32_in`. Donguru release flash (text) 258,340 → 246,780 B (−11,560 B), data 724 B unchanged, same embassy fork commit and `opt-level` in both builds (`llvm-size`). Not run on a board.
- `Cargo.lock` of the workspace and `fw/donguru` on wire_weaver 0c18ed2 and exact_units 1819d9f, `fw/donguru` on the embedded_bedrock commit with the same API change (`cargo update -p bedrock_sync -p bedrock_led -p bedrock_crash` again once embedded_bedrock is on main) and a newer `embassy-stm32` of the fork branch.

## [0.20.43] - 2026-11-04

- `fw/donguru`: the romixlab/embassy fork dependencies and patches take `branch = "donguru-ring-copy"` instead of `rev = 84ae135...`; Cargo.lock pins the commit, and `cargo update -p embassy-stm32` stays on the patched line. The next build needs that update in `fw/donguru`

### Changed

- Dependencies on sibling repos (wire_weaver, embedded_bedrock, pd_bench) are git dependencies without rev, pinned by `Cargo.lock`, instead of `../` paths; the `[patch]` blocks pointing at sibling checkouts are commented out in `Cargo.toml` and `fw/donguru/Cargo.toml` (uncomment for local development). The next build needs `cargo update -p wire_weaver` (and `-p embedded_bedrock`, `-p pd_bench`) in the root and in `fw/donguru`.

## [0.20.42] - 2026-11-03

### Changed

- Firmware without the feature `meter-samples` (the default image) takes no samples at all (FW-65, WireWeaver SRV-56 `observe_samples = false`): no batcher, outbox or `Series` encoder. Release flash (text + data) 289,580 → 266,404 B (−23,176 B), RAM −3,392 B. `donguru_api` and the wire don't change: `meter.reading` stays `+batched`, a host sees the same API, and its other modes send the same bytes. Images with `meter-samples` keep the same sizes. Not run on a board.
- `Samples` of `meter.reading` on such a firmware is refused with `ModeNotSupported`, where it used to be granted with no rows ever sent. `PowerMeter::observe_samples` answers `Error::Unimplemented`, so `dg power watch --samples` and Python's `samples=True` fail with a clear message instead of waiting forever.
- The firmware says it takes no samples when a host connects (WireWeaver `ImageFacts`, SRV-56), so `PowerMeter::observe_samples` fails without asking the device. The refusal stays the answer for a host that doesn't read it. The image with `meter-samples` grows by 256 B of flash for it, and the same-base numbers are in FW-65.

## [0.20.41] - 2026-11-03

### Changed

- Firmware: the `plug.pd` message log ring (`donguru::pd::LOG`) moved from `.data` to `.bss` by giving its unused
  slots a dedicated zero-tag `Kind::Empty` instead of a real event, so the ring's static initializer is all-zero
  (FW-64). Release text+data 289,780 B → 287,508 B (−2,272 B); `.data` 3,052 B → 724 B. No wire change (`Kind` is
  firmware-internal). Tested on B153A: `dg fw update`, then `dg pd log` over a real negotiation.

## [0.20.40] - 2026-11-03

### Added

- CLI: `donguru device reset-cause` prints a crash's call chain as `file:line` frames, the panic site first, decoded with the firmware's ELF found by the record's build id or given with `--elf FILE` (`--force` to use one whose build id differs); `--format json` has it as `call_chain` (`null` without addresses, `{"error": ...}` when not decoded). Library: `donguru_core::crash::frames` over wire_weaver's `ww_report::crash` (CLT-59), `Error::Elf`. Tested on B153A with a panicking test image (FW-63).

### Changed

- Firmware built against wire_weaver master 61c095a with SRV-53 (global trait paths by hash; was 512f59e): release flash (text + data) 314,356 B → 311,940 B (−2,416 B). No source or `Cargo.lock` changes (FW-63).
- Firmware on the pinned nightly `nightly-2026-11-02` (FW-63, embedded_bedrock TPL-81, the same as donguru_template): `fw/donguru/rust-toolchain.toml` with rustfmt, clippy, rust-src and the target. Release flash 311,940 B → 317,824 B (+5,884 B, the nightly's prebuilt `core`: std 15.7 → 18.6 KiB in cargo bloat), won back by the next steps. The dev build overflows FLASH by 408 B on it.
- No home or registry paths in the firmware's flash (FW-63): `trim-paths = "all"` in the dev and release profiles of `fw/donguru/Cargo.toml`. Panic locations name `src/...`, `/cargo/deps/<crate>-<version>/src/...`, `/cargo/git/<hash>/<rev>/<crate>/src/...` and `/cargo/registry/<hash>/<crate>-<version>/src/...`; only debug info (`.debug_str`, not flashed) still names the embedded_bedrock checkout. Release flash 317,824 B → 314,904 B (−2,920 B); the dev build fits again, 514,748 B of 516,096 B.
- Release firmware without panic locations (FW-63): `-Zlocation-detail=none` for the release profile only (`[profile.release] rustflags` with `[unstable] profile-rustflags` in `fw/donguru/.cargo/config.toml`); dev builds keep them. Release flash 314,904 B → 293,912 B (−20,992 B). A panic message now reads `panicked at <redacted>:0:0`; the crash record's return addresses (embedded_bedrock ROB-13) name the site through the ELF instead.
- `core` built with the firmware (FW-63): `build-std = ["core"]` with `build-std-features = ["optimize_for_size"]` in `fw/donguru/.cargo/config.toml`, so core is compiled at opt-level z inside fat LTO instead of the prebuilt one at O3. Release flash 293,912 B → 289,780 B (−4,132 B). The default dev build no longer fits: it overflows FLASH by about 17.6 KB with core at the dev profile's opt-level 3 and debug assertions, and turning features off does not help (all off: 17.3 KB over). Only the release has to fit; `CARGO_PROFILE_DEV_OPT_LEVEL=s cargo run` builds a dev image that does (444,864 B, assertions and overflow checks kept), see `fw/donguru/README.md`.

## [0.20.39] - 2026-11-03

### Added

- `donguru_api` gets WireWeaver's evolution snapshot test (API-63): `just test` / `just api-check`
  now catch an API change that needs a bigger version bump than it got, the way `donguru_api`'s break during
  STD-98 was found only by hand. `just api-save` saves a version's snapshot; 0.6.0's is the first, committed in
  `donguru_api/api_snapshots/`.

## [0.20.38] - 2026-11-03

### Changed

- Built against WireWeaver master 512f59e (was c440491). No source changes needed: meter.rs already used `Prefix::pow10()` (STD-59), and the board's reworks stay omitted in `ww_codegen!` (STD-101). Release firmware text+data is 314,088 B, down from 316,884 B (−2,796 B), and 15,048 B below 241ff78 from before the flash diet (329,136 B). SRV-51 alone had added 14,220 B (f5f88ea: 331,572 B); SRV-55's shared reply paths take that back. Per crate: donguru −17,715 B and ww_device −410 B, ww_client_server +14,029 B (the shared dispatch now lives there). Not run on a board.

## [0.20.37] - 2026-11-03

### Fixed

- Firmware: the PD source no longer misses a sink's GoodCRC while the USB server is busy, which made it send about 1 message in 30 twice with `pd log` streaming. Its task runs on an interrupt executor of its own (FW-62).
- Firmware: after a hard reset the PD source waits its VBUS settle time again before Source_Capabilities; they went out 3 ms after VBUS came on (FW-60).
- CLI: `donguru pd log -o FILE` keeps capturing for its `--for` when the screen output is closed (`| head`); it used to end at the first screen line and leave the file empty. In M1 that made a loop's `--restart`s come back to back, so a sink's Request got no GoodCRC (FW-56).

### Known bugs

- A reserved message type from the sink gets Not_Supported without a GoodCRC first: usbpd 3.0.0 drops it before the acknowledgement (FW-61).

## [0.20.36] - 2026-11-03

### Added

- CLI: `donguru pd log [--follow] [-o FILE] [--restart] [--for SECONDS]` turns `plug.pd.messages` into `pd_log::Record`s (pd_bench LOG-1, path dependency) and writes them as JSON lines with the host time to `-o FILE`. On screen it prints the same decode as `pd-log print` (usbpd's real parser through `pd_log::Decoder`). `pd-log print` and `pd-log merge` read the file and line a Donguru log up with a rePort sink's by host time (CLI-55, API-30). Tested on B153A against the ZY12PDN.

### Changed

- CLI: `donguru pd log` needs `--follow` or `--for SECONDS` (it used to follow forever), and `--format jsonl` is replaced by `-o FILE` with `pd_log` records. An active `sender_response` override is noted once on stderr. The header-guessing `donguru_core::pd::describe` is removed (CLI-55).

### Known bugs

- `pd log --restart` with a `sender_response` override loses the first contract: the log is empty and the sink's Request gets no GoodCRC. Recorded, not fixed (FW-56).

## [0.20.35] - 2026-11-03

### Changed

- `device.board.set_reworks` (`ww_board_info` 0.3.0, STD-98) added to `omit` next to `device.board.REWORKS` in the
  firmware (`fw/donguru/src/ww/mod.rs`) and the host mock server (`donguru-core/src/mock/server.rs`): reworks aren't
  served yet (STD-24); keeps building once STD-98 lands on wire_weaver main.
- Built against wire_weaver master 084f9e9 with its rework guard (STD-101), which has no API items any more: the guard runs in a firmware's `main` before any init and refuses a board with a rework the image doesn't know. Donguru keeps no reworks list (STD-24), so its firmware doesn't run the guard: nothing to omit beyond `device.board.REWORKS` and `set_reworks`. The host's `#[since]` check now compares versions in full (wire_weaver 425b7477), so `device.boot`, `device.dfu` (0.5.1) and `plug.pd` (0.5.2) keep their markers across the 0.6.0 bump. Tested 3 Nov 2026 on B153A (a dev PC): flashed through the ROM bootloader (`ww dfu stm32 flash`, as the 0.6.0 CLI refuses the 0.5.2 firmware), boots as `donguru_api@0.6.0`, `ww info` answers, `dg fw version`, `pd status`, `power status`, `settings show` work, and `dg fw update` writes it again (38.8 s).
- `donguru_api` bumped 0.5.2 → 0.6.0 (Breaking): wire_weaver STD-98 changed `ww_board_info::Rework`'s `id` from a
  string to a number, which changes `Device::board`'s wire signature even though `device.board.REWORKS` is
  omitted here — a device built against 0.6.0 doesn't talk to a host built against 0.5.x or older, and the other
  way around.
- Firmware: the release build is at `opt-level = "z"` (FW-57): 322.0 KiB of FLASH instead of 501.7 KiB, to leave room for the full PD stack. USB loopback on B153A is 10-25 % slower than at opt 3 (1 KB: 253 round trips/s, 662 / 550 kB/s each way; opt 3: 296, 830 / 555); no API change. Header UART LZ4 and the meter were not measured.

## [0.20.34] - 2026-11-02

### Added

- Firmware: a USB PD source on the plug CC, `plug.pd` (API-30, `donguru_api` 0.5.2, a patch bump: wire-compatible, old CLIs keep connecting and don't see the level). `plug.pd.source` offers one 5 V PDO at a chosen current (100 mA to 3 A) with the matching Rp, `plug.pd.state` says attach / contract / no PD / failed, `plug.pd.messages` streams every message both ways raw with the device time in µs, plus attach, detach, VBUS and hard-reset events. usbpd 3.0.0's source policy engine on UCPD2; plug VBUS on only after a debounced Rd, guarded at 5.5 V and the offer plus 25 %, off at the detach and in a hard reset. `sender_response` is an out-of-spec test knob for slow sinks. While the source runs, `plug.vbus_mode`, `vbus_cycle` and `debug.ucpd_role` on UCPD2 are refused. Tested on B153A with the ZY12PDN's FUSB302B driven by hand over rePort I2C, no load: contract at the spec's timers, Reject for a bad Request, detach and re-attach with USB up, `NoPd` for a sink without PD, hard reset from the sink. The stock ZY firmware acknowledges Source_Capabilities but sends no Request (API-30 stays open for that check).
- CLI: `donguru pd source on [--current A] [--sender-response-ms N]`, `pd source off` and `pd status` drive the PD source (API-30); exit 4 on a firmware without `plug.pd`. Library: `Plug::pd_source`, `set_pd_source`, `pd_state` and `donguru_core::pd::source`. The mock models the setting and the state.
- CLI: `donguru pd log [--restart] [--for SECONDS]` follows `plug.pd.messages` with device time and message names, `--format jsonl` for scripts; `--restart` restarts the source to show a negotiation from the attach on (API-30).

### Changed

- Firmware: five features are cargo features, off in the default build, so the PD source fits in flash (FW-53): `debug-api` (12.4 KiB), `swd-host` (SWD-host and the CMSIS-DAP interface, 14.2 KiB), `header-uart` (`uart[1]`, 10.8 KiB), `api-docs` (doc comments in `ww introspect`, 13.4 KiB) and `meter-samples` (`meter.reading` in `Samples` mode, 13.0 KiB). A default image has no CMSIS-DAP interface and no header UART, `debug` answers Unimplemented. Release FLASH 500.20 -> 501.79 KiB of 504 KiB with the PD source in (569.13 KiB with it and all five), RAM 91.02 -> 63.66 KiB of 144 KiB. Build with `--features swd-host` (or any other) for an image that needs one.
- Build: the root workspace and the firmware patch `shrink_wrap` to the WireWeaver checkout and pin exact_units to WireWeaver's own rev, so both build against today's checkout again.

### Fixed

- Bug recorded, not fixed: firmware built before 27 Oct 2026 can't be reached by today's `ww` or `dg`, so `just update` fails on it; the way out is in FW-55.

## [0.20.33] - 2026-10-25

### Added

- External subcommands: `donguru NAME` runs `donguru-NAME` from `PATH` with `DONGURU_DEVICE` / `DONGURU_OUTPUT` set, unknown commands get a "did you mean" hint (exit 2), `donguru help` lists plugins (CLI-4).
- `donguru config generate / show / path / validate` and layered configuration: system, user and project `donguru.toml`, `DONGURU_DEVICE` / `DONGURU_OUTPUT` / `DONGURU_COLOR`, flags; the `device`, `output` and `color` keys (CLI-9). Not run against a board: no hardware involved.
- `donguru udev generate / install / uninstall / status` (CLI-8): the rule covers the dongle and the STM32 ROM bootloader; install and uninstall ask for root through `pkexec` or print the `sudo` command. Not yet run as root on a real system.
- `donguru swd orientation [auto|normal|swapped]` and `swd release` (CLI-17), `donguru led mode / brightness / identify` (CLI-21): mock-tested, not yet on a board.
- An item the dongle's firmware doesn't implement yet ends with "the device's firmware does not support this yet", exit code 4 as before (CLI-17, CLI-21).

### Fixed

- The mock dongle refuses a guarded VBUS mode write while VBUS is over `max_voltage` (`Refused`, nothing changes) instead of tripping, like the firmware's `donguru_board::power` (CORE-6).

## [0.20.32] - 2026-10-25

### Added

- Python package `donguru` (`donguru-py/`, PyO3 + maturin): `donguru.list()`, `donguru.open("bench-1")`, power, meter (read, `watch()` as an iterator, totals), USB data, header, UART configure, settings, LEDs, firmware version and info, as a blocking API with SI floats; exceptions per error category; `donguru.mock.MockDevice` to test scripts without a dongle; type hints. `just py-test` runs pytest against the mock (also part of `just test`), `just py-build` builds the wheel. Tested against the mock only, not on a board yet (CORE-4).
- Docs page [Python](docs/user/python.md) (CORE-4).
- Pick a dongle by its user label: `--device bench-1` (when no serial matches) or `--device label:bench-1`, any case; the "matches none" / "matches 2" errors list the labels (CORE-1, for CORE-4's `donguru.open("bench-1")`).

## [0.20.31] - 2026-10-25

### Added

- A spinner on stderr while `power cycle --off` (1 s or more, counting down), `device log --follow` and `device counters --watch` wait; hidden with `-q` or when stderr is not a terminal, stdout unchanged. Not run on a board yet (CLI-53).

### Changed

- `swd memtest` shows its spinner also under `--format json` / `jsonl` (on stderr, only on a terminal); its output is unchanged (CLI-15).
- `--format json` / `jsonl`: a failed command writes `{"error": {"exit": "no_device", "code": 3, "message": "...", "kind": "refused"}}` as one line on stderr instead of `donguru: error: ...`; stdout stays data only, the exit code is unchanged; `kind` only when the dongle answered with an error. Text output keeps the old line. `docs/user/scripting.md` documents formats, exit codes and the error object (CLI-52).
- Text output of `power`, `header`, `usb`, `uart`, `info`, `device`, `settings`, `fw` and `swd memtest` is coloured on a terminal (keys, values, states: on / connected green, tripped / fault red); `--color never`, `NO_COLOR` or a pipe give the same text as before. json and jsonl are unchanged (CLI-15, CLI-51).
- Exit codes settled as `src/exit.rs` has them, now the documented contract: 0 success, 1 failure, 2 bad arguments, 3 no device, 4 device error, 5 timeout; each has a stable name (`no_device`, ...). `developer-notes.md` and `docs/design/cli` no longer list a transport code; a broken link stays 1. No exit code changed (CLI-13).

## [0.20.30] - 2026-10-25

### Added

- `just package` builds `.deb` and `.rpm` packages of `donguru`/`dg`: man pages for every command (`clap_mangen`),
  bash/zsh/fish completions for both binary names, and a udev rule granting unprivileged USB access to the dongle,
  with a `dg` symlink and udev reload wired into each package's install/remove scripts. Built as CI artifacts on
  every run, not published anywhere yet. Docs: `docs/user/installation.md` (PKG-3).

### Changed

- `donguru-cli` and `donguru-core` now set `license.workspace = true` like every other workspace crate; the
  workspace's `license = "TBD"` is the one place packaging metadata and `Cargo.toml` both read it from (PKG-4).

## [0.20.29] - 2026-10-25

### Added

- Firmware: the `debug` level (API-18), behind `debug-api` (on by default): `pin_set` / `pin_get` / `pin_af` by MCU pin with the refused list (NRST, supply pins, our USB and SWD, CC_DA_EN, the VBUS enables, the hub's reset and I2C), `InUse` for pins a running function holds (a claim table in `donguru_board::pins`: LEDs, meter, button, header UART, V_SBU select, an SWD session and its plug CC drive, a UCPD role) and `Refused` for pin states that fight on the board (tied nets, SBU_OE with PA0 / PA1, PA0 / PA1 push-pull without V_SBU, `~PWR_FLT` push-pull); `pins` and `boot_pins` (GPIOA..F captured before init); `hub_read` / `hub_write` over SMBus through the hub watchdog task (STATUS/COMMAND refused); `ucpd` / `ucpd_role` (Rp on UCPD1 refused on rev A); `led_set`; `reg_read` in peripheral blocks only. A CMSIS-DAP `DAP_Connect` releases the SBU pins first and claims them for the session. No `donguru_api` change (the items were declared at API-8): old CLIs keep working. Release FLASH 425.41 -> 444.00 KiB of 504 KiB (88.09 %), RAM 89.83 -> 90.58 KiB of 144 KiB. Built, not run on a board.
- `dg debug pin get|set|af PIN` (net name or port pin, completed with the net list), `dg debug pins [--boot] [--diff]`, `dg debug hub read|write`, `dg debug ucpd [1|2] [--role]`, `dg debug reg ADDR [N]`; refusals exit 4 with the firmware's reason (CLI-23). `donguru_core::debug`: pin decode, the diff against the reset state, UCPD decode. Docs: Bring-up and debug.

### Changed

- The mock's `debug` level runs the firmware's own rules (`donguru_board`): refused pins, claims, electrical rules, AF 0..=7 (was 0..=15), `hub_read` 1..=32 bytes, real UCPD_CR bits, `reg_read` in peripheral blocks (API-18).

## [0.20.28] - 2026-10-25

### Changed

- Firmware: built against WireWeaver with TR-15 (setup token in link setup, framer session reset), a wire break: firmware from here on talks only to hosts with TR-15 (wire_weaver 689b697a and the CLI built on it), older firmware not to them. No `donguru_api` change. Release FLASH 425.24 KiB of 504 KiB (84.4 %), RAM 89.83 KiB of 144 KiB; the last figure in this changelog was 407.04 KiB / 89.27 KiB (FW-44), main itself did not build to measure. Not run on a board yet (FW-51).
- `just lint` also checks the firmware (`just fw-lint`: fmt check, clippy with default, all and no default features) (FW-23).

### Fixed

- The firmware builds again: `bedrock_crash` from the embedded_bedrock checkout next to this repo instead of its git, WireWeaver's git source patched to the checkout next to this repo so the bedrock crates and the firmware share one `ww_indication` (FW-51).
- Firmware clippy clean (FW-23).

## [0.20.27] - 2026-10-22

### Added

- `just test --changed` runs only the workspace crates changed against main and their dependents (`tpm test changed`, nextest's `rdeps()`); plain `just test` runs everything, as `just test-all` does (tpm ops#22oct-2049).

## [0.20.26] - 2026-10-22

### Changed

- `just update` and the firmware-update docs use `ww dfu stm32 ...` (wire_weaver renamed `ww stm32dfu` under `ww dfu`; the old name still runs as a hidden alias for now) (FW-12).

## [0.20.25] - 2026-10-22

### Changed

- `just install` builds with the `deploy` profile; with tpm it installs through the kept-releases stack in ~/.local/bin (the `dg` link beside it), without tpm it runs plain `cargo install` (link in the cargo bin dir). `just deploy` works without tpm and checks `donguru --version`

## [0.20.24] - 2026-10-22

### Changed

- Firmware: the shared clock follows one steerer at a time (bedrock_sync DEV-13, wire_weaver SRV-31): while one program's steer points keep coming, another one's are only noted, and it takes over after 3 s without the first; `clock.sync_status` names the followed (`steerer`) and the noted program (`noted`). No API change on Donguru's side (`ww_timebase` fields are `#[default = None]`). (FW-44)

## [0.20.23] - 2026-10-21

### Changed

- Firmware: the hub stays attached and configured through a firmware update, a crash or a reboot (FW-46), so a device on the plug keeps its USB link through a reboot or a crash; only Donguru's own USB device re-enumerates. At boot the firmware reads the hub's config back over I2C and keeps the hub when it holds what this firmware writes and is attached; a firmware with another hub config, or a hub that lost its config, gets the reset pulse and the config as before. A power cycle still starts the hub from reset. New counters `hub_kept` and `hub_kept_reconfigured`. No API change (`donguru_api` stays 0.5.1). Tested on B153A (a dev PC, 21 Oct 2026): the hub stayed the same host device through a probe flash, 3 `ww dfu reboot --into app` and a ROM update (`just update`); a firmware with a changed hub config reconfigured it, both ways; brown-out recovery (FW-18) still works; with a self-powered USB serial DUT on the plug (B129A) its open tty kept echoing through 5 reboots in a row, none lost. Not kept on rev A: the DUT's link through a ROM update, because the ROM bootloader drives the plug data switch off (PA9 is its USART1_TX, FW-50), and a bus-powered DUT, because plug VBUS goes off at every MCU reset (FW-49).

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

- Firmware: a boot loop ends in the STM32 ROM bootloader on its own (FW-45), so a firmware that crashes as it starts can still be updated over USB without a probe. Every boot counts in the boot word (TAMP_BKP4R); a boot is healthy once USB is configured or after 10 s up (a dongle on a charger never trips it). After 3 boots in a row without that, the fourth brings the hub up, resets into the ROM's USB DFU (0483:df11, the same serial) and `ww stm32dfu -s SERIAL flash ELF` writes a good image. The next firmware logs it and reports `device.dfu.node` `last_result` as `Recovery` until the next requested update or a power cycle (was always `Nothing`; no API change, `donguru_api` stays 0.5.1). Cargo feature `crash-loop-test` builds an image that panics at every start, to check it. Tested on B153A (a dev PC, 21 Oct 2026) without a probe: the crash-loop image written over USB ended in the ROM DFU behind the hub about 1 s after it started, `ww stm32dfu flash` brought the good image back with `Recovery` reported, the next `just update` cleared it, 5 app reboots in a row stayed in the firmware. Not run on a board: the rescue right after a power cycle, the 10 s mark without a USB host.

## [0.20.19] - 2026-10-21

### Added

- `dg fw version` shows the running firmware (name, version, git commit, profile, API version) and `dg fw update
  [ELF]` writes a firmware over USB without a probe: reboots into the STM32 ROM bootloader, writes and verifies,
  waits for the firmware and prints the old and the new version, with a progress bar on stderr and `--format json`
  (CLI-10). `donguru_core::update` has the typed API (`FirmwareImage`, `Dongle::update_firmware`,
  `Device::reboot`). Tested on B153A (a dev PC, 21 Oct 2026): three updates in a row, 44 s each.

## [0.20.18] - 2026-10-21

### Added

- Firmware update over USB without a probe, through the STM32 ROM bootloader (FW-12, API-58): `device.boot`
  (`ww_dfu::BootControl`) and `device.dfu.node` in `donguru_api` 0.5.1, a wire-compatible addition (older CLIs keep
  working with this firmware, this CLI with older firmware). `ww dfu -s SERIAL reboot` puts the dongle into the ROM's
  USB DFU behind its hub, `ww stm32dfu -s SERIAL flash ELF --wait 10 --follow` writes it and waits for the firmware
  to come back; `just update SERIAL` does both, `docs/user/firmware-update.md` explains it. Needs a `ww` from
  wire_weaver master (`ww stm32dfu`). On rev A the hub's RESET_N is driven from RTC_OUT1 so the hub stays up through
  the resets. Tested on B153A (a dev PC, 21 Oct 2026): ROM DFU up in under 0.5 s, 415 KB written and verified in
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
  to steer without wobbling UART / PWM, decided 21 Oct 2026, from our internal design notes). The status LEDs
  (`src/led.rs`) align their breath / blink to `SYNC`'s shared time while it has one (`bedrock_led::Indication::
  run_with`), free-running otherwise as before, so Donguru's LEDs breathe in phase with other boards synced by the
  same host (`ww sync`). Release FLASH 391.59 → 407.04 KiB of 504 KiB (+15.45 KiB), RAM 89.11 → 89.27 KiB of
  144 KiB (+0.16 KiB). Flashed and run on B153A (a dev PC): boots clean, 227 s with no panic; `ww sync` end to
  end not yet exercised, blocked this session by the board's USB hub stuck from concurrent unrelated use of the
  same board (FW-44).

## [0.20.16] - 2026-10-21

### Changed

- Firmware and CLI built against WireWeaver master with TR-15 (link setup token, per-medium framing, 2bd3cc95): a wire break of the link, not of `donguru_api` (still 0.4.1). A dongle on older firmware no longer talks to this CLI and older CLIs don't talk to this firmware: flash the firmware and install the CLI together. Tested 21 Oct 2026 on B153A (a dev PC, flashed by J-Link): the new CLI against the 0.20.14 firmware stops with "The device speaks link 0.5.0 (without setup token) ... update the older side" (exit 1); after the flash `dg info`, `device list`, `device reset-cause`, `device counters --elf`, `power status`, `power meter`, `power watch` and `settings show` work, 30 reconnects in a row all connect, counters all 0.

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
- `MeterReading` has no `power` any more (`donguru_api` 0.4.0): it was always `vbus_in` x `current`, now computed on the host (`donguru_core::reading_power_w`); `dg power` shows mW and `power_mw` in JSON as before. Saves a column of every batched meter sample on the wire and its encoder in the firmware. Update firmware and host together: hosts with `donguru_api` 0.3 can't observe or read `meter.reading` of a 0.4 firmware and the other way round (the API version tells them apart, `dg` says so). Release FLASH at opt-level 3 363.1 -> 355.7 KiB (-7.4), at z 246.4 -> 243.7 KiB (-2.8). Tested 19 Oct 2026 on B153A: `dg power status` / `watch` show mW as before.
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

- Docs: product names written WireWeaver and ShrinkWrap in prose.

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
  `ww-elf` Syncthing share between two of our PCs (`tpm area/ops/provision.toml`), approved and
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
- `dg device list` and `dg device info` (CLI-7, text / json / jsonl). Tested 15 Oct 2026 on B153A:
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
  14 Oct 2026 on B153A (J-Link via a dev PC): HUB-1 (config/attach) and HUB-5 (VID/PID readback, now
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
  carries SWD, a stopgap until FW-8. Never on the power-cut bench (the DUT runs from Q102 there).

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
  and `swd_release` Hi-Z disconnect, for the power-cut bench (FW-15, API-4). The pins also go Hi-Z on a
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
