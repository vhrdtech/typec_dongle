# HANDOFF: bringup-debug (API-18 + CLI-23), session s/25oct2026-bringup-debug

Task: P2534#15oct-2044f. Plan: tpm prj/P2534-donguru/design/api-v1.md ("debug" section + "Night plan"), decisions in night-24oct-decisions.md (add ours there, one line per paragraph). No board, no probe: never flash; FEATURES 🧪 + check-on-B153A list.

## Done (committed)

- `donguru_board/src/pins.rs`: refused list (`refused`), `bonded` (44 LQFP48 pins, PE none), `Holder` (Leds, Meter, Button, HeaderUart, VSbu, Swd, SwdCc, Ucpd1, Ucpd2) with `name()` (<=32 B for `InUse { by }`) and `pins()`, `Claims` (`boot()`, `claim` all-or-nothing, `release`, `holder`), `Violations` (tied nets PB1/PB3, PB6/PB9/PB11, PB7/PB8; OE PB2 high with PA0/PA1 driving; PA0/PA1 push-pull without PD3 high; PC14 push-pull), `Change`/`plan`/`PinWrite::apply` (write order ODR, OTYPER, PUPDR, AFR, MODER), `check(pin, change, claims, ports)`, `reset_state()` (RM0444: all analog, PA13 AF/PU/very-high, PA14 AF/PD), `check_reg_read` (1 KiB peripheral blocks from stm32-metapac g0b1ce, word aligned, 1..=32), `HUB_REG_STATUS_COMMAND` 0xFF (hub_write refuses it).
- `donguru_board/src/ucpd.rs`: `role_fields(port, role, rev_a)` (Rp refused on UCPD1 rev A), `role_of(cr)`, `holder(port)`.
- Baseline `just fw-size` (before): FLASH 425.41 / 504 KiB (84.41 %), RAM 89.83 / 144 KiB.

## Next

1. Firmware (`fw/donguru`):
   - new `src/pins.rs`: static `CLAIMS` (critical_section Mutex<RefCell<Claims::boot()>>), `capture_boot()` first thing in `main` (before `boot::early`): enable all GPIO clocks raw (`RCC.gpioenr()` bits 0..5, metapac lacks GPIOEEN), read GPIOA..F, restore GPIOENR; `read_ports()`, `dump()`, `set(pin, change)` in one critical section (read ports, `check`, write fields via pac, return PinState incl. IDR).
   - `src/cmsis_dap.rs`: before processing DAP_Connect call `host.release()`; after a successful connect claim Swd (cnt! on failure), claim SwdCc and only then `drive_cc()` (else cnt!). `SwdHost::release()` releases Swd + SwdCc.
   - `src/hub.rs`: debug SMBus requests served by `watchdog_task` (third select arm), request/response Signals with a sequence number; handler awaits with timeout (Busy/Timeout); block read = write_read(DEVICE_ADDR 0x2C, [reg], buf[1+len]) return data[1..]; block write = write([reg, len, data..]); embassy i2c Error::Nack -> DebugError::Nack, Timeout -> Timeout.
   - `src/ww/debug.rs`: implement pin_set/pin_get/pin_af/pins/boot_pins/hub_read/hub_write/ucpd/ucpd_role/reg_read behind `cfg!(feature = "debug-api")`; add `debug.hub_read|debug.hub_write` to `handler_model` async list in `ww/mod.rs`; buffers in ServerState (`[PortRegs; 6]`, `[u8; 32]`, `[u32; 32]`). Denied -> CommonError (Invalid{arg,reason}, Refused{reason}, InUse{by: holder.name()}).
   - UCPD: enable APB clock (RCC apbenr1 ucpd1en/ucpd2en), CFG1 (PSC DIV2, hbitclkdiv 12, transwin 7, ifrgap 16, ucpden) before CR (anamode/anasubmode/ccenable both); CC pins analog; claim Ucpd1/Ucpd2 (InUse -> Refused "CC pins in use"); Off releases, ccenable off, ucpden off. `rev_a` from feature `rev_a`.
   - `just fw-lint`, `just fw-size` after -> CHANGELOG.
2. donguru-core: mock (`src/mock/mod.rs` `check_pin`/`set_pin_mode`, server.rs debug handlers) -> use `donguru_board::pins::check` + `Claims::boot()` + `reset_state()` for boot_pins; add donguru_board dep to donguru-core. Core: `debug` diff against reset state (ignore IDR), net-name lookup already in `src/nets.rs`, UCPD decode (role_of, vstate text).
3. CLI `dg debug pin get|set|af NAME`, `pins [--boot] [--diff]`, `hub read|write`, `ucpd [1|2] [--role]`, `reg ADDR [N]`; insta snapshots vs the mock; refused -> clear message + exit code (see `src/exit.rs`); docs usage page. CLI-21 (`led`) and CLI-17 (`swd`) belong to session cli-plumbing: don't add `dg led`.
4. FEATURES (API-18, CLI-23 🧪 + check list: every allowed pin set/read back, relay clicks on PA5, `dg debug pins --boot --diff` clean, hub VID/PID via hub_read, ucpd states), CHANGELOG [Unreleased] with fw-size before/after; decisions file in tpm; `just test --changed`, `just lint`; rebase on main; `git rm HANDOFF.md`; `tpm work ready --note ...`; `tpm work say parent` 3 lines; then `tpm work resume cli-output@omarchy-m1` and end. Spawn nothing.

## Decisions to write into night-24oct-decisions.md

- PD3 (V_SBU select) held by `VSbu` for good until API-14 exposes io_level; SBU pins free for debug outside an SWD session, under the electrical rules; DAP_Connect first releases SBU (undoes debug changes).
- AF counts as driving (conservative) in the tied-net and OE rules.
- hub_write refuses 0xFF (STATUS/COMMAND: reset/attach drop our own USB); use hub.recover.
- reg_read only inside mapped 1 KiB peripheral blocks (a reserved address would fault); reads of data registers may consume data (documented).
- PA5/PA9/PA10 (relay, data cut, SBU_TO_USB) free for debug although `power`/`hub` set them: the API state (`header.relay`, `plug.data`) may then disagree until the next write there.
