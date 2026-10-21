# Handoff: donguru-tr15

Done:
- typec_dongle f7b752f: Cargo.lock for wire_weaver master 2bd3cc95 (TR-15); fw dev + release build, `just test` and `just lint` pass. No code change was needed (the Framing type change sits inside wire_weaver_usb_embassy). CHANGELOG [Unreleased] says it's a link wire break.
- embedded_bedrock ecd866f (session branch): bedrock_template manifests renamed to `Cargo.toml.liquid`. The git dependency never failed, cargo only printed parse errors (exit 0).

Next:
- Waiting on Roman's OK (asked via tpm work say parent) to flash the donguru on omarchy-m1 and test the new CLI against it (`dg` list/info, uart). Then record the board test in CHANGELOG, `git rm HANDOFF.md`, `tpm work ready`.

Gotchas:
- cargo-generate rendering of the `.liquid` manifests is not tested here (cargo-generate isn't installed); the template is superseded (TPL-20).
