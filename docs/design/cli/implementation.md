---
icon: lucide/wrench
---

# CLI Implementation

This page turns the [CLI design](index.md) into concrete implementation
choices: the language and crates, the module/crate layout, how configuration is
layered in code, output and error handling, testing, and packaging. It is a
living document — as libraries are chosen and validated, decisions here are
updated.

## Language and toolchain

- **Language: Rust** (edition 2021 or later). Rationale: a single static
  binary per platform, strong cross-compilation story (Linux → macOS →
  Windows), good USB/serial crates, and it shares an ecosystem with the
  firmware/`wireweaver` side.
- **MSRV**: pin a minimum supported Rust version and test it in CI. *TBD.*
- **Build tasks** are exposed through the existing `justfile` alongside the
  docs tasks.

## Crate selection

The following crates are the initial selection. Each row notes the job and
why it is preferred over alternatives.

| Concern              | Crate                         | Why                                                                 |
| -------------------- | ----------------------------- | ------------------------------------------------------------------- |
| Argument parsing     | **`clap`** (derive)           | De-facto standard; derive API keeps the command tree declarative; built-in help, subcommands, env fallback. |
| Shell completions    | **`clap_complete`**           | Generates completions for the `completions` subcommand from the same `clap` model. |
| External subcommands | `clap`’s `allow_external_subcommands` | Enables the Git/Cargo-style plugin dispatch without extra deps.     |
| Config files         | **`figment`** (or `config`)   | Layered providers (defaults → files → env) matching the design’s precedence; merges TOML + env cleanly. See [figment vs config](evaluations/config-figment-vs-config.md). |
| Serialization        | **`serde`** + **`toml`**      | Config parsing and `--format json` share one model.                 |
| JSON output          | **`serde_json`**              | `json` and `jsonl` output formats.                                  |
| USB access           | **`nusb`**                    | Pure-Rust, cross-platform, async-friendly USB; avoids a libusb C dependency. |
| RPC / wire format    | **`WireWeaver`** | Matches the dongle’s control-channel protocol.                     |
| Errors (app)         | **`anyhow`**                  | Ergonomic error propagation at the binary boundary.                 |
| Errors (library)     | **`thiserror`**               | Typed errors in the core library crate that map to exit codes.      |
| Logging/diagnostics  | **`tracing`** + `tracing-subscriber` | Structured diagnostics to stderr, controlled by `-v`/`-q`.   |
| TTY / colour         | **`anstream`**/`anstyle` (+ `is-terminal`) | Auto-disable colour when piped; honour `NO_COLOR`.          |
| Tables (text output) | `tabled` or `comfy-table`     | Aligned human output for `list`/`info`. See [tabled vs comfy-table](evaluations/tables-tabled-vs-comfy-table.md). |
| Async runtime        | **`tokio`**        | Required by WireWeaver, blocking interface provided via channels for both Rust and Python.             |

!!! note

    Crate choices are decisions of record but not irreversible. Prefer the
    pure-Rust option (`nusb`) first to keep cross-compilation simple; fall back
    to libusb-backed `rusb` only if a platform gap appears.

## Workspace and crate layout

Split the front end from the reusable core so subsystems are libraries and the
binary is thin. This also makes the transport mockable for tests.

```text
cli/
├── Cargo.toml            # workspace
├── crates/
│   ├── donguru-core/     # library: transport trait, RPC calls, domain types
│   │   ├── transport/    #   wireweaver-over-USB impl + mock impl
│   │   ├── gpio.rs
│   │   ├── usb.rs        #   hub port control
│   │   ├── power.rs
│   │   ├── i2c.rs
│   │   ├── uart.rs
│   │   ├── fw.rs
│   │   └── device.rs     #   enumeration + selection
│   └── donguru-cli/      # binary: clap tree, config, output, exit codes
│       ├── main.rs
│       ├── cli.rs        #   clap derive command tree
│       ├── config.rs     #   figment layering
│       ├── output.rs     #   text/json/jsonl renderers
│       └── commands/     #   one module per subsystem, calls into core
└── ...
```

- **`donguru-core`** knows nothing about `clap`, argv, or exit codes. It exposes
  a `Transport` trait and typed operations. This is where the wireweaver RPC
  lives, behind the trait.
- **`donguru-cli`** owns argument parsing, config resolution, output rendering,
  and mapping core errors to process exit codes.
- The `dg` alias is produced by installing the same binary under both names
  (e.g. `[[bin]]` targets or an install-time symlink). *TBD which mechanism.*

## Configuration layering in code

Implement the design’s precedence:

- Paths come from platform conventions via **`directories`** (or `etcetera`)
  so macOS/Windows resolve correctly.
- Project config is found by walking up from the CWD looking for
  `donguru.toml`/`.donguru.toml`.
- The merged `Configuration` deserializes into one `Config` struct (`serde`).

## Output and formatting

- A global `--format/-f {text,json,jsonl}` flag (default `text`) selects a
  renderer. Commands return typed values; `output.rs` renders them.
- Streaming commands (`gpio watch`, `power watch`) emit `jsonl` naturally and
  flush per record.
- Colour via `anstream`/`anstyle`, disabled unless stdout is a TTY; respect
  `--color=auto|always|never` and `NO_COLOR`.
- Diagnostics go to stderr through `tracing`; `-v` raises the level, `-q`
  lowers it. stdout stays pure data.

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

- User-facing error messages go to stderr and are concise, `-v` adds the error
  chain/backtrace.

## External subcommand dispatch

- Enable `clap`’s external subcommand capture. When the first argument is not a
  known subcommand `X`, search `PATH` for `donguru-X` and `exec` it.
- Forward remaining args verbatim and pass resolved context via environment
  (e.g. `DONGURU_DEVICE`, `DONGURU_OUTPUT`) so plugins inherit selection and
  format.
- `donguru help` augments its listing by scanning `PATH` for `donguru-*`
  executables.

## Testing

- **Unit tests** in `donguru-core` against a **mock `Transport`** — no hardware
  needed, assert the RPC calls each operation issues.
- **CLI tests** with **`assert_cmd`** + **`predicates`** for argument parsing,
  exit codes, and stdout/stderr separation; **`insta`** snapshots for `--help`
  and text/json output.
- **Config tests** for the layering precedence (defaults vs file vs env vs
  flag) 

            use `figment`’s `Jail` for isolated env/CWD?


- **Hardware-in-the-loop** tests gated behind a feature/CI runner with a
  dongle attached. *TBD.*

## Packaging and distribution

- Build per-platform static binaries in CI (Linux first, then macOS, then
  Windows). Use `cargo dist` *(TBD)* or hand-rolled release jobs.
- Ship shell completions generated by `clap_complete` and a man page
  (`clap_mangen`) alongside the binaries.
- Linux permissions: document the **udev rule** needed for non-root USB access
  to the dongle.

## Related evaluations

Library choices above are backed by focused comparisons in the
[CLI evaluations](evaluations/index.md) section:

- [`figment` vs `config`](evaluations/config-figment-vs-config.md)
- [`tabled` vs `comfy-table`](evaluations/tables-tabled-vs-comfy-table.md)

## Open items

- Async vs. blocking transport (drives whether `tokio` is a dependency).
- `nusb` coverage on Windows; fall back to `rusb`/libusb if required.
- Table crate selection (`tabled` vs `comfy-table`).
- Firmware update payload transport over wireweaver.
- Finalised exit-code numbering and env-var names (freeze before 1.0).
