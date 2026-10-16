# Handoff: cli-info (CLI-7 + CLI-19)

Session `cli-info`, branch `s/16oct2026-cli-info`, worktree `/tmp/wt/cli-info/typec_dongle` (sibling
`/tmp/wt/cli-info/wire_weaver` is the path dependency checkout). Task: P2534#15oct-2044h.

## Done (committed: `feat(cli): device info/select/label/log/counters/reset-cause (CLI-7, CLI-19)`)

- `dg info` (top-level): same contract summary as `dg device info` (firmware, board/variant, uid, label,
  last reset), short no-noun spelling. `donguru-cli/src/commands/info.rs`.
- `dg device select SELECTOR`: resolves against connected dongles, pins the **serial** (not whatever form was
  typed) in `./donguru.toml`'s `device` key. `donguru-cli/src/project_config.rs` (read/write, walks up to find the
  file, always writes to CWD). `Context::from_cli` reads it as the lowest-priority `--device` fallback.
- `dg device label [NAME] [--notes TEXT]`: get/set `UserInfo` name + notes. Added `Device::notes`/`set_notes` to
  `donguru-core/src/levels.rs` (the generated client already had `read_notes`/`write_notes`, core didn't expose
  them yet).
- `dg device reset-cause [--clear]`: `LastReset`'s cause/message/boot count/crash record, `--clear` calls the new
  `Device::clear_last_reset`. Added `Device::boot_count`/`clear_last_reset` to levels.rs.
- `dg device counters [--watch]` and `dg device log [--follow]`: new `donguru-core/src/counters.rs` and `src/log.rs`
  modules. Decode `ww_counters`/`ww_log` with the firmware's ELF found by its build id, using `cnt_core` +
  `defmt-decoder` directly (NOT `ww_report`: that crate's `Contract`/`DynClient` machinery is for a
  traits-only/global-attachment connection; we already know our fixed absolute path (`device.counters`,
  `device.log`), so the generated `DonguruApiDeviceClient` plus the same lower-level utility crates `ww_report`
  itself uses is simpler and avoids a second connection or `DynClient::from_cmd` wrapping).
  - Key design point: when there's nothing to decode (`counters.INFO.ram.used_words == 0` and no backup buffer;
    `log`'s `frames_status().session == None`), we return empty **without touching the ELF at all**. This is both
    the right behavior (no need to open an ELF for zero counters / no log yet) and what makes these two paths
    testable against the mock without a real firmware-shaped ELF fixture — `cnt_core::Counters::load_elf` and
    `defmt_decoder::Table::parse` both hard-error on an ELF that was never linked with `cnt.x`/defmt, so a
    throwaway host binary doesn't work as a stand-in once you actually try to decode something.
- Mock (`donguru-core/src/mock/`): now models `last_reset`/`boot_count`/`clear` (new `MockState` fields:
  `last_reset_cause`, `last_reset_message`, `last_reset_boots_since_crash`, `boot_count`) and `label_notes`
  (fixed `DEVICE_LABEL_CAPABILITIES.notes_max` from 0 to 256 to match real firmware per FEATURES.md API-9).
  `counters`/`log`'s own RPCs (`read`, `reset`, `frames`) are still `Unimplemented` in the mock, matching real
  firmware until API-10 (defmt log) and more `cnt!` counters land — this is intentional, not a gap I'm leaving by
  accident.
- `donguru-cli/src/main.rs` used to duplicate the whole module tree (`mod cli; mod commands; ...`) instead of using
  the `donguru_cli` library crate. Fixed to `use donguru_cli::{cli, commands, context, exit};` — this wasn't just
  cleanup, it was causing every insta snapshot to be generated twice under two different crate-name prefixes
  (`donguru__...` for the bin, `donguru_cli__...` for the lib), which fails `cargo test` because only one set gets
  created unless you update both. If you add more unit tests with snapshots, you should only ever see
  `donguru_cli__...` files now.
- `docs/design/cli/index.md` rewritten: full v1 command tree from `~/git/tpm/prj/P2534-donguru/design/api-v1.md`
  (not just this piece's slice — api-v1.md's line "Changes to docs/design/cli/index.md go with CLI-7's piece"
  means CLI-7 owns writing the whole spec, even commands other pieces implement later). Removed the stale "Open
  questions" section, added an "Exit codes" table matching `src/exit.rs` exactly (the design doc had drifted;
  `developer-notes.md` is still wrong per the existing bug CLI-13 — I didn't touch that file, only made sure my
  new doc doesn't repeat its mistake). Device selection section now describes the real selector grammar
  (`index:`/`serial:`/`path:` prefixes, bare-form heuristics) and the `donguru.toml` project file.
- FEATURES.md: CLI-7 and CLI-19 moved from 🚧/📋 to 🧪 (built and mock-tested, **not yet run on hardware** — per
  AGENTS.md, ✅ for anything touching hardware needs a board test first).
- CHANGELOG.md: `[Unreleased]` entries added for all of the above.
- New deps: `cnt_core`, `defmt-decoder` (donguru-core); `toml`, `ww_firmware_info`, `insta` (dev), `tempfile` (dev)
  (donguru-cli).
- `just verify` (fmt-check + clippy -D warnings + test, `--all-features --locked`) is green.

## Not done / next

1. **Hardware round** (this task's "Done" criterion): run every new command on the B153A on omarchy-m1 with
   `--format text` and `--format json`, check exit codes against `src/exit.rs`. The board is shared (see the
   task's original brief: "say before and after you flash", `tpm work ls`/`tpm work say`). Expect:
   - `device label`/`reset-cause` should work for real (API-9 is ✅ on hardware already).
   - `device counters`/`device log` will very likely still answer "not implemented" for the RPCs themselves
     (API-10 for log isn't built; the firmware's `cnt!` counters may or may not have non-zero `used_words` —
     check `device.counters.INFO` first with `ww introspect`/`ww counters info` before assuming). If counters
     *does* have used words > 0 on real firmware, this is the first real end-to-end test of the ELF-by-build-id
     decode path — watch for it actually finding the firmware ELF (check `~/.cache/ww/elf`, `WW_ELF_PATH`, or
     `target/` of the firmware build) and decoding real names.
   - After the hardware round, flip CLI-7/CLI-19 to ✅ in FEATURES.md with the board revision, update the CHANGELOG
     tested-on note.
2. Everything else in the v1 CLI tree (power/usb/header/sbu/uart/i2c/gpio/adc/input/hub/led/swd/fw/debug/settings)
   is **out of scope for this piece** — those are later pieces per api-v1.md's table (CLI-6, CLI-11, CLI-16,
   CLI-20..24, CLI-10, CLI-5, CLI-12, CLI-17, CLI-21, CLI-22, CLI-23). Don't implement them here.
3. `device counters --watch` is a simple poll loop (re-reads and reprints every `--interval`); there's no
   diff/changed-only logic like `ww counters watch` has. Good enough for v1 per the design line
   (`dg device counters [--watch]`), but worth knowing if someone compares it to `ww counters watch`.
4. Didn't add a `reset` subcommand to `device counters` (the design tree doesn't list one for `dg device
   counters`, unlike `ww counters reset`) — intentional, not an oversight.

## Gotchas if you touch this code again

- To see the generated client's exact method/type names for a new WireWeaver item, temporarily uncomment
  `debug_to_file = "../target/donguru_client.rs"` in `donguru-core/src/client.rs`, `cargo check -p donguru-core
  --features mock`, read `target/donguru_client.rs`, then revert the comment. Much faster than guessing.
- `ww_report` (the crate `ww info`/`ww report`/`ww counters` are built from) is the right reference for *patterns*
  (ELF resolution, defmt decode, counters decode) but its `Contract`/`clients.rs` machinery is for a *generic*
  traits-only connection via `DynClient` + global attachment — we don't need that since our own generated client
  already knows the fixed path. Don't pull `ww_report` itself in as a dependency; borrow the pattern, use
  `cnt_core`/`defmt-decoder` directly, as `counters.rs`/`log.rs` already do.
- `MaxLen<T, N>` is one type used both borrowed (`T = RefVec<'i, _>` / `&'i str`) and owned (`T = Vec<_>` /
  `String`) — there's no separate `MaxLenOwned`. `MaxLen::new_unchecked(vec![...])` / `.new_unchecked(string)` for
  constructing an owned one in tests.
- `donguru-core`'s `read`/`call`/`call_plain` helpers in `levels.rs` are `pub(crate)` now (were private) so
  `counters.rs`/`log.rs` can reuse them — keep that if you refactor levels.rs.
