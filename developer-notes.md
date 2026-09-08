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

- Core returns typed `thiserror` errors; the binary maps them to stable exit
  codes so scripts can branch on failure category:

| Code | Meaning                                   |
| ---- | ----------------------------------------- |
| 0    | Success                                   |
| 1    | Generic/unexpected error                  |
| 2    | Usage error (bad arguments) — `clap`      |
| 3    | No matching device / ambiguous selection  |
| 4    | Transport error (USB/RPC failure)         |
| 5    | Device reported an error                  |
| 6    | Timeout                                   |

  (Exact numbers to be finalised, keep them documented and stable once shipped.)

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
