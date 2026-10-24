# Handoff: cli-output (CLI-13/15/51/52/53)

Paused by the user on 24 Oct 2026 at a consistent point (builds, clippy clean, tests green).

## Done
- CLI-13 exit codes: exit.rs is the contract (0..5), `ExitCode::code()` / `name()`, a test pins them; developer-notes.md and docs/design/cli/index.md follow it. ✅ in FEATURES.
- CLI-15 renderer: `donguru-cli/src/output.rs` (`Render` trait: text with Theme, json, jsonl records; `Output::emit` / `record`; `render()` for tests; `report_error` / `error_json`). Ported: power, header, usb, uart, info, device (list/info/select/label/log/counters/reset-cause). JSON shapes and uncoloured text byte-identical (existing insta snapshots unchanged, new ones added).
- CLI-51 theme: `theme::Theme` (key, heading, ok, warn, bad, value, dim) and `Painted` (pads visible text, not escape codes) in theme.rs.
- CLI-52 (code done, docs not): main.rs reports errors through `output::report_error`: text line as before, json/jsonl one `{"error":{exit,code,message,kind?}}` line on **stderr** (stdout stays data, per docs/design/cli "stdout is data, stderr is diagnostics"). `Error::kind_name` / `DeviceErrorKind::name` in donguru-core.

## Next
1. Port settings.rs, fw.rs (version, update report), swd.rs (memtest: replace its private `Paint` with `Theme`), udev/config only if cli-plumbing hasn't reworked them (check main first).
2. CLI-52 docs: write docs/user/scripting.md (formats, exit-code table with names, the error object, jsonl streams); add "Errors under json" section to docs/design/cli/index.md (the exit-code section already links `#errors-under-json`). Record the stderr decision in ~/git/tpm/prj/P2534-donguru/design/night-24oct-decisions.md.
3. CLI-53: spinner on stderr (hidden unless text-free stderr TTY and not -q) for `power cycle --off` (show remaining off time), `device log --follow` ("waiting for log lines"), `device counters --watch` (between reads). Reuse swd.rs `spinner` shape; maybe move to output.rs.
4. FEATURES: CLI-15/51/52/53 ✅ with pointers; CHANGELOG entries extend the existing [Unreleased] ones.
5. Rebase onto main (FEATURES/CHANGELOG conflicts: keep both), `just test --changed`, `just lint`, `git rm HANDOFF.md`, `tpm work ready`, `tpm work say parent`.

## Gotchas
- `Painted` defaults to left alignment (also for numbers): write `{:>8}` explicitly.
- serde_json has no preserve_order: key order is sorted, so building json differently doesn't change output.
- `on_reports_a_trip_as_failure` test prints a line to stdout (old behaviour, harmless).
