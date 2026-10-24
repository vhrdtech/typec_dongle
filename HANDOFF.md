# cli-plumbing handoff (paused 24 Oct 2026 night)

Done and committed: CORE-6 (mock VBUS guard refuses), CLI-17 (swd orientation/release), CLI-21 (led mode/brightness/identify), plain "firmware does not support this yet" message for Unimplemented (exit.rs `message`).
In progress (checkpoint commit, builds): `donguru-core/src/udev.rs` (rule text incl. the STM32 DFU 0483:df11 line, install/remove/status with tests against tempdirs/fake sysfs). The CLI `udev` commands in `donguru-cli/src/commands/udev.rs` are still stubs.
Next:
1. CLI-8: wire `udev generate/install/uninstall/status` onto core::udev. Install: try write; on PermissionDenied re-run self via `pkexec <current_exe> udev install ...` (one prompt), else print the exact `sudo` command; reload with `udevadm control --reload-rules` + `udevadm trigger` (unless --no-reload). Never run install on this PC; test generate/status only. FEATURES CLI-8 -> 🧪, CHANGELOG, decisions note (DFU line included, pkexec self re-exec).
2. CLI-9 config generate/show/path/validate with layering (docs/design/cli/index.md "Configuration"); touches context.rs and project_config.rs.
3. CLI-4 external subcommands: PATH lookup `donguru-NAME`, env `DONGURU_DEVICE` / `DONGURU_OUTPUT` (pick the design doc's names, fix the doc comment), near-match hint, list in `donguru help`.
4. Decisions to write in tpm prj/P2534-donguru/design/night-24oct-decisions.md (not yet written): udev DFU line; pkexec self re-exec; led mode name `all-off` per design.
Gotchas: Cargo.lock shows churn from the local wire_weaver checkout: do not commit it. Before ready: rebase on main, `just test --changed`, `just lint`, `git rm HANDOFF.md`, `tpm work ready`.
