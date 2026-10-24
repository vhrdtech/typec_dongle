# HANDOFF: bringup-debug (API-18 + CLI-23), session s/25oct2026-bringup-debug

Task: P2534#15oct-2044f. Plan: tpm prj/P2534-donguru/design/api-v1.md ("debug" section + "Night plan"), decisions in night-24oct-decisions.md (add ours there, one line per paragraph). No board, no probe: never flash; FEATURES 🧪 + check-on-B153A list.

## Done (committed)

- `donguru_board/src/pins.rs`, `ucpd.rs` (commits 8133737, 09c2277): refused list, `bonded`, `Holder`, `Claims` (`boot()`, `claim`, `release`, `holder`), `Violations`, `Change`/`plan`/`check`, `PinWrite::apply`, `reset_state()`, `check_reg_read`, `REG_READ_MAX_WORDS`, `HUB_REG_STATUS_COMMAND`; `ucpd::role_fields(port, role, rev_a)`, `role_of(cr)`, `holder(port)`. Unit-tested.
- Firmware (commit e66db15), `just fw-lint` green (all three feature sets):
  - `fw/donguru/src/pins.rs`: `CLAIMS` (embassy blocking Mutex), `capture_boot()` first thing in `main` after `log::init_rtt`, `dump()`, `boot()`, `get()`, `set()` (check + raw writes in one lock), `claim`/`release`/`holds`, `set_analog`.
  - `src/ucpd.rs`: `state(port)` (zeros while the APB clock is off), `set_role` (claims CC pins, analog, clock, CFGR1 as embassy, CR bits, SYSCFG strobe set and left set).
  - `src/hub.rs`: `SmbusRequest`/`SmbusResponse`/`SmbusError`, `smbus(request)` async with 100 ms timeout and a seq number, served as the third `select3` arm of `watchdog_task` (`Hub::smbus`: block read write_read [reg] -> [count, data..]; block write [reg, len, data..]).
  - `src/swd.rs`: `SwdHost::connected()` claims Swd (cnt on failure) and SwdCc (drive_cc only when that succeeds); `release()` floats the CC pins only when it held SwdCc, then releases both. `src/cmsis_dap.rs`: `host.release()` before processing DAP_Connect, `host.connected()` after a good one.
  - `src/ww/debug.rs`: all handlers implemented behind `cfg!(feature = "debug-api")`; `DebugState` buffers in `ServerState.debug`; `denied()` maps `Denied` to CommonError; SMBus Bus error -> Busy; ucpd_role InUse -> Refused "CC pins in use by an SWD session". `debug.hub_read|debug.hub_write` async in `handler_model`.
  - Added dep `critical-section = "1.2"` to fw Cargo.toml.
- `just fw-size`: before FLASH 425.41 / 504 KiB (84.41 %), RAM 89.83 / 144 KiB; after FLASH 444.00 KiB (88.09 %), RAM 90.58 KiB (62.90 %). +18.6 KiB flash: the handlers' serializers, PinDump ser (1.6 K), pins::set (1.6 K), the server task's grown state. Goes into CHANGELOG.

## Next

1. donguru-core mock (step 2, started, nothing changed yet): `donguru-core/src/mock/mod.rs` has its own `check_pin`, `set_pin_mode`, `invalid_pin`, `RESET_PIN`, `reset_dump`, `ucpd()` (fake CR numbers) and `MockState.pins: [[PinState;16];6]`, `hub_regs`, `ucpd_role`; `mock/server.rs` lines ~1397-1580 the debug handlers (imports at line 12-13). Replace with donguru_board: add `donguru_board = { path = "../donguru_board" }` to donguru-core deps; mock keeps `pins` but computes `Ports` from it, runs `donguru_board::pins::check(pin, change, &claims, &ports)` and writes the `PinWrite` back (input follows output for outputs); `MockState.claims: Claims` (pub, `Claims::boot()`), plus Swd claimed while `swd_session` is Connected; boot_pins from `reset_state()` (idr 0); hub_read len 1..=32, hub_write refuses 0xFF; reg_read via `check_reg_read`; ucpd via `role_fields` with real CR bits (ANAMODE bit 9, ANASUBMODE 8:7, CCENABLE 11:10) so `role_of` decodes them. Fix `donguru-core/tests/mock.rs` (~line 440-480) to match. Core `Debug` facade (`levels.rs` 829-895) already has every call; add a diff helper (dump vs `reset_state()`, IDR ignored) and UCPD decode (role_of, vstate text) in core.
2. CLI `dg debug pin get|set|af NAME`, `pins [--boot] [--diff]`, `hub read|write`, `ucpd [1|2] [--role]`, `reg ADDR [N]` (design: api-v1.md ~line 554); no debug command module yet in `donguru-cli/src/commands/` (see `gpio.rs` / `swd.rs` for the pattern, `cli.rs` for the tree, `exit.rs` codes); net names from `donguru_core::nets` (`lookup`, `pin`, `port_pin`, `net_of`, `port_pin_name`); insta snapshots vs the mock; refused -> clear message + exit code; docs usage page. Don't add `dg led` (CLI-21) or `dg swd` (CLI-17): cli-plumbing's.
3. FEATURES (API-18, CLI-23 🧪 + check list: every allowed pin set/read back, relay clicks on PA5, `dg debug pins --boot --diff` clean against verify_b153a.md §0, hub VID/PID via hub_read, ucpd states read), CHANGELOG [Unreleased] incl. fw-size before/after and the firmware pieces above; decisions into night-24oct-decisions.md; `just test --changed`, `just lint`; rebase on main; `git rm HANDOFF.md`; `tpm work ready --note ...`; `tpm work say parent` 3 lines; then `tpm work resume cli-output@omarchy-m1` and end. Spawn nothing.

## Decisions to write into night-24oct-decisions.md

- PD3 (V_SBU select) held by `VSbu` for good until API-14 exposes io_level; SBU pins free for debug outside an SWD session, under the electrical rules; DAP_Connect first releases SBU (undoes debug changes).
- AF counts as driving (conservative) in the tied-net and OE rules.
- hub_write refuses 0xFF (STATUS/COMMAND: reset/attach drop our own USB); use hub.recover.
- reg_read only inside mapped 1 KiB peripheral blocks (a reserved address would fault); reads of data registers may consume data (documented).
- PA5/PA9/PA10 (relay, data cut, SBU_TO_USB) free for debug although `power`/`hub` set them: the API state (`header.relay`, `plug.data`) may then disagree until the next write there.
- UCPD2's SYSCFG strobe is set with its first role and left set (clearing it would bring the dead-battery pull-down back); `Off` only turns CCENABLE and UCPDEN off.
- An SMBus bus/arbitration error answers `Busy`; a hub NAK answers `Nack`; no answer from the watchdog task in 100 ms answers `Timeout`.

## Gotchas

- `cnt!` can't be a match-arm expression: wrap it in `{ cnt!(..); }`.
- metapac has GPIOE but no GPIOEEN field: raw `gpioenr` bits 0..5.
- `PscUsbpdclk::Div2` (not DIV2) in this metapac.
- FEATURES.md / CHANGELOG.md not updated yet for the firmware commit: do it in step 3's commit.
