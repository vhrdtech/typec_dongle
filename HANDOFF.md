# PKG-3 packaging — handoff

Session `pkg`, tpm task P2534#24oct-2333d. Context-size handoff mid-step (tpm's own prompt,
`tpm work handoff`), not a user pause — just continuing in a fresh conversation on the same branch.

Queue note still in force: Roman's slot B allows max 2 sessions at once, this session is last in it.
**When the work below is done: `just test --changed` + `just lint`, then `tpm work ready --note "..."`
and `tpm work say parent "..."`, then end the turn. Don't spawn any subagents/workflows this session
(explicit instruction from the user) — do all remaining steps directly.**

Also: main has moved twice since this branch started (fw-night's v0.20.28, bringup-debug's v0.20.29).
Nothing to do about that now — `tpm land` rebases at landing and FEATURES.md/CHANGELOG.md/Cargo.lock
merge themselves. Only rebase by hand now if a conflict actually blocks `just test`/`just lint`
(`tpm work rebase`), not pre-emptively.

## Done so far (commits d1a0419, 022d9b8 on this branch)

- `xtask` crate (new workspace member): `cargo run -p xtask -- target/dist/man` walks the whole CLI
  tree with `clap_mangen::generate_to` and writes 60 man pages (`donguru.1`, `donguru-device.1`, ...).
  Verified it builds and runs.
- `license.workspace = true` added to `donguru-cli` and `donguru-core` — every other workspace crate
  already had it, this was a plain gap, not a deliberate omission. Root `LICENSE` placeholder file
  added explaining the TBD status (PKG-4) so `license-file` has real content to point at.
- `packaging/60-donguru.rules`: static vendor udev rule (`SUBSYSTEM=="usb"`, `idVendor==c0de`,
  `idProduct==cafe`, `TAG+="uaccess"`), independent of the live `donguru udev` command (CLI-8's
  `generate`/`install`/`uninstall`/`status` are still 100% stubs — `donguru-cli/src/commands/udev.rs`,
  FEATURES.md's CLI-8 note claiming `generate` works is stale; CLI-8 belongs to the parallel
  `cli-plumbing` session tonight, don't touch `udev.rs`).
- `packaging/deb/postinst` / `postrm`: symlink `/usr/bin/dg -> donguru` on install, remove it on
  remove/purge, `udevadm control --reload-rules`/`trigger` either way (both executable, `chmod +x`
  already applied — verify that survived `git add` with `git ls-files -s packaging/deb/` showing mode
  100755, not 100644).
- `packaging/dg.1`: a short hand-written man page redirecting to `donguru(1)` (avoids duplicating the
  whole 60-page tree under a second name for what is just a symlink).

## Not yet done (in order — this is the part to actually execute now)

1. **`donguru-cli/Cargo.toml` metadata** — add:
   - `[package.metadata.deb]`: `maintainer`, `license-file = ["../LICENSE", "0"]` (path is relative to
     `donguru-cli/`, one level up to the repo root — verify this resolves; cargo-deb docs say asset
     `source` paths get special `target/release/` handling but `license-file` may want a path relative
     to the *workspace* root instead — check cargo-deb's actual behavior for workspace members before
     trusting either guess, the research done before the first pause did not nail this down),
     `depends = "$auto"`, `section = "utility"`, `priority = "optional"`,
     `maintainer-scripts = "../packaging/deb"` (same relative-path caveat as above),
     `assets` — needs, at minimum:
     - `["target/release/donguru", "usr/bin/", "755"]`
     - `["target/dist/man/*.1", "usr/share/man/man1/", "644"]`
     - `["../packaging/dg.1", "usr/share/man/man1/", "644"]`
     - `["target/dist/completions/bash/donguru", "usr/share/bash-completion/completions/", "644"]`
       (and the `dg` counterpart, and zsh `_donguru`/`_dg` into
       `usr/share/zsh/site-functions/`, and fish `donguru.fish`/`dg.fish` into
       `usr/share/fish/vendor_completions.d/`) — 6 completion files total, see step 2 for how they're
       generated.
     - `["../packaging/60-donguru.rules", "usr/lib/udev/rules.d/", "644"]`
   - `[package.metadata.generate-rpm]`: same asset list in that tool's `{ source, dest, mode }` form,
     `post_install_script` / `post_uninstall_script` as inline strings mirroring the deb postinst/postrm
     content (RPM scriptlets, not separate files). `license` is read from `package.license`
     automatically (now "TBD" via the workspace inheritance added above) — don't duplicate it unless
     the tool errors without an explicit override.
   - All these **relative path guesses above need verifying against the installed tools' actual
     behavior**, not just trusted from memory — that's exactly what step 4 (local end-to-end run) is
     for. Expect to iterate on this file once `cargo deb`/`cargo generate-rpm` are actually invoked.

2. **`justfile` recipe(s)** (`[group: 'package']`), simple orchestration only, logic stays in the
   tools/xtask already written:
   ```
   just build the release binary once (cargo build --release --locked -p donguru-cli)
   mkdir -p target/dist/completions/{bash,zsh,fish}
   for NAME in donguru dg; for SHELL in bash zsh fish: `target/release/donguru completions $SHELL --bin $NAME > target/dist/completions/$SHELL/<name-per-shell-convention>`
   cargo run -p xtask -- target/dist/man
   check `cargo deb --version` / `cargo generate-rpm --version` exist, same pattern as cargo-bloat/cnt
   in `just fw-size` (print install hint, exit 1 if missing)
   cargo deb --no-build -p donguru-cli
   cargo generate-rpm -p donguru-cli
   ```
   Keep the shell loop to the "~10 lines, a few commands" ceiling the user's CLAUDE.md sets for
   justfile recipes — if the completions generation needs more than that, it still counts as "a few
   commands" (one loop over 2 names x 3 shells, no parsing/branching logic) but double-check it reads
   that way once written; if it balloons, consider a tiny step in xtask instead (xtask already exists,
   cheap to add a second small helper there rather than stretch the justfile recipe).
   Note the exact filename conventions per shell:
   - bash: plain name, e.g. `usr/share/bash-completion/completions/donguru` (no extension)
   - zsh: `_NAME` prefix, e.g. `usr/share/zsh/site-functions/_donguru`
   - fish: `NAME.fish`, e.g. `usr/share/fish/vendor_completions.d/donguru.fish`

3. **CI job** (`.github/workflows/ci.yml`): new job, `cargo install --locked cargo-deb
   cargo-generate-rpm` (cache the cargo-install'd binaries the same way other CI steps cache — check
   what caching, if any, the existing `verify` job uses before adding a new pattern), run the `just
   package` recipe from step 2, `actions/upload-artifact` on the `.deb` and `.rpm` outputs (locations:
   `cargo deb` defaults to `target/debian/*.deb`, `cargo generate-rpm` defaults to
   `target/generate-rpm/*.rpm` — confirm both once step 4 has actually run them, don't guess the
   upload-artifact `path:` from memory). No publish step anywhere.

4. **Verify locally, end to end** — this is the step that will surface wrong guesses above:
   ```
   cargo install --locked cargo-deb cargo-generate-rpm   # network confirmed working earlier this session
   just package   # or whatever the recipe ends up named
   dpkg-deb -c target/debian/*.deb   # inspect file list/perms without installing (dpkg-deb should be present on this box; if not, `dpkg --contents` or just unpack with ar/tar)
   rpm2cpio target/generate-rpm/*.rpm | cpio -t   # inspect rpm contents similarly, if rpm2cpio is available; otherwise note what couldn't be checked
   ```
   Fix whatever the metadata guesses in step 1 got wrong. A full `dpkg -i`/`rpm -i` install test needs
   root and pollutes the host system — do the content-listing check above instead unless there's a
   disposable container handy; note in FEATURES.md exactly which level of testing was actually done
   (content listing vs. real install) rather than overclaiming.

5. **Docs page**: `docs/user/installation.md` (new), covering what the packages install (binary, `dg`
   alias, man pages, completions, udev rule) for end users, linking the existing
   `docs/design/packaging/index.md` design doc for the "why". Add it to `zensical.toml`'s `nav` under
   `Documentation`. Mention the still-TBD license and that `.deb`/`.rpm` are CI artifacts, not yet
   published anywhere (no apt/dnf repo, no GitHub Releases upload — that's conscious scope, not a gap
   to silently leave undocumented).

6. **FEATURES.md**: update PKG-3 (currently 📋 at line ~934) to 🧪 — built, not installed on a real
   machine (clarify exactly what was and wasn't verified per step 4's findings) — with a pointer to
   `xtask/`, `packaging/`, the `donguru-cli/Cargo.toml` metadata tables and the new `just` recipe. Also
   touch PKG-4 (TBD license, line ~943) with a one-line pointer to the new `LICENSE` file and
   `license.workspace = true`, since that's a side effect of this work worth recording even though
   PKG-4 itself isn't "done" (still genuinely TBD, just now consistently wired through Cargo metadata).

7. **CHANGELOG.md**: one `### Added` entry under `[Unreleased]` covering the packages, man pages,
   completions and udev rule (PKG-3), and a short aside on the license metadata fix (PKG-4) — these
   can be one entry or two, whichever reads more naturally once written.

8. **Gates and handover**: `just test --changed` and `just lint`; fix anything that breaks (the new
   `xtask` crate is covered by `cargo check --workspace`/`clippy --workspace` even though it has no
   tests of its own — don't skip linting it). Then `git rm HANDOFF.md`, `tpm work ready --note "..."`,
   `tpm work say parent "..."`, end the turn. Do not land, push, or run `tpm work done` — that's
   Roman's call once he's back.

## Gotchas already found (don't re-derive)

- `cargo-deb` builds the project itself by default; pass `--no-build` once step 2 already built, to
  avoid compiling twice. `cargo-generate-rpm` never builds — always `cargo build --release` first.
- `cargo-generate-rpm -p donguru-cli` tries asset `source` relative to the **current directory first**,
  then the package directory. Run both tools from the **workspace root**, not `cd`'d into
  `donguru-cli/`, so `target/dist/...` and `packaging/...` paths resolve the same way for both tools.
- `donguru_core::device::USB_VID` / `USB_PID` = `0xc0de` / `0xcafe` (`donguru-core/src/device.rs`) —
  already used correctly in `packaging/60-donguru.rules`; don't let a later edit drift from these.
- FEATURES.md's CLI-8 note is stale (claims `generate` works; the code is 100% stub). Don't rely on
  it, don't fix it here — that's `cli-plumbing`'s session tonight.
- Neither `nfpm`, `cargo-deb`, `cargo-generate-rpm`, nor `clap_mangen` (as a binary) are preinstalled on
  this box. `cargo-deb`/`cargo-generate-rpm` install fine via plain `cargo install` (confirmed network
  access to github.com works). `xtask` already depends on the `clap_mangen` *library*, so that one
  doesn't need a separate install.
- `donguru-cli` depends on `probe-rs` by default (its own Cargo.toml turns the feature on), so building
  it — and therefore `xtask`, which depends on it as a lib — pulls that in regardless; this was already
  true before this session's changes, not something introduced here.
