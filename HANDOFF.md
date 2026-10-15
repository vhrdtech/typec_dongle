# HANDOFF: defmt-log (API-10)

## Done (commit 9ab3632, wip)

- `fw/donguru/src/log.rs`: `ww_defmt_logger::global_logger!(LOG, 4096)` replaces defmt-rtt (kept as a dev-dependency
  for the examples); RTT channel `defmt` (1 KiB, NoBlockSkip) as the tee, set first thing in main.
- `device.log.frames` store: `PersistentRamRing<8192>` in `.uninit.donguru_log`, DropOldest; at boot the stored
  session continues, else a new one (`boot_count << 16 | 1`). `ServerState::new` and `ww::start` are async now.
- Server task: wakes every 10 ms (`log::PUMP_PERIOD`) or 1 ms while observing; pumps the ring, then
  `poll_streams` (2048 B budget, 528 B item buffer); detaches the log stream on link down.
- `device.log.DEFMT` served as a fn (`const_model` adds `DEFMT`): encoding at run time, build id.
- `debug.log_flood(count)` added to `Bringup`, `#[since = "0.2.1"]`, donguru_api 0.2.0 -> 0.2.1 (additive).
- Builds dev and release. Release size: FLASH 244.03 -> 264.70 KiB (+20.7), RAM 16.15 -> 33.35 KiB (+17.2)
  (base measured in a temp worktree of 8c36da7).

## Next

1. `git rebase main` once local main has 25a45b9 (core-client): donguru-core's mock server
   (`donguru-core/src/mock/server.rs`) then needs a `debug_log_flood` handler; `just verify`.
2. Host side for the test: no `ww log` yet. Either an example in donguru-core using its new donguru_api client
   (resume `device.log.frames` from pos 0, decode with defmt-decoder; like wire_weaver
   `examples/defmt_log/examples/defmt_log.rs`), or the start of CLI-19.
3. Hardware on omarchy-m1 (say to firmware sessions before/after flashing): boot log over USB and RTT at once;
   `log_flood 1000` -> exact lost count; log a line, wait > 10 ms, force a HardFault via J-Link (pc 0xFFFFFFF0 as in
   the FW-13 test), reconnect: the line is there after the reset.
4. FEATURES API-10, CHANGELOG (fw-size delta, api 0.2.1, RTT now rtt-target: probe-rs may set it blocking),
   bedrock_fw.json nuance (defmt-rtt -> ww_defmt_logger), report to donguru-api and wire_weaver STD-26's owner.

## Gotchas

- Opening the stream anew (not resume) resets the store: hosts must resume to get the pre-connect / pre-crash log.
- Lines logged after the last pump (the fault handler's own) are lost at a reset; the crash record has the fault.
- Paused by Roman for the night (15 Oct): no flashing until resumed.
