# Cli Implementation Notes

##  Dependencies

- [clap](https://crates.io/crates/clap)
- [clap_mangen](https://crates.io/crates/clap_mangen)
- [clap_complete](https://crates.io/crates/clap_complete)
- [config](https://crates.io/crates/config)
- [thiserror](https://crates.io/crates/thiserror)
- [anyhow](https://crates.io/crates/anyhow)
- [nsub](https://crates.io/crates/nsub)
- [toml](https://crates.io/crates/toml)
- [serde](https://crates.io/crates/serde)
- [serde_json](https://crates.io/crates/serde_json)
- [wireweaver]()
- [tokio](https://crates.io/crates/tokio)


## Output and formatting

- A global `--format/-f {text,json,jsonl}` flag (default `text`) selects a
  renderer. Commands return typed values; `output.rs` renders them.
- Streaming commands (`gpio watch`, `power watch`) emit `jsonl` naturally and
  flush per record.
- Colour is disabled unless stdout is a TTY. respect `--color=auto|always|never` and `NO_COLOR`.
- Diagnostics go to stderr through `tracing`; `-v` raises the level, `-q` lowers it. stdout stays pure data.

## Error handling and exit codes

- Core returns typed `thiserror` errors; the binary maps them to stable exit codes so scripts can branch on the
  failure category. The table and the mapping live in `donguru-cli/src/exit.rs`, documented in
  `docs/design/cli/index.md` ("Exit codes") and `docs/user/scripting.md`; a new category gets the next free number,
  an existing one is never renumbered.

- User-facing error messages go to stderr and are concise, `-v` adds the error chain/backtrace.

## External subcommand dispatch

- Enable `clap`’s external subcommand capture. When the first argument is not a
  known subcommand `X`, search `PATH` for `donguru-X` and `exec` it.
- Forward remaining args verbatim and pass resolved context via environment (e.g. `DONGURU_DEVICE`, `DONGURU_OUTPUT`)
  so plugins inherit selection and format.
- `donguru help` augments its listing by scanning `PATH` for `donguru-*` executables.

## Testing
- UnitTest
- Insta

## Packaging and distribution

- Static Binary/ies
- Man page(s)
- Shell completions
- Udev rules (for linux)
