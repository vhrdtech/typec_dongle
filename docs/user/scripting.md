---
icon: lucide/file-code-corner
---

# Scripting

`donguru` (or `dg`) is meant to be driven from shell scripts and test benches: every command can print JSON, and its
exit code says what went wrong. This page is the contract scripts can rely on. From Python, use the `donguru`
package instead, see [Python](python.md); the Rust interface (`donguru-core`) is not documented yet.

## stdout is data, stderr is diagnostics

Only the result a command was asked for goes to stdout. Warnings, notes, progress bars and spinners, and errors go
to stderr, so `dg ... | jq` never sees anything but data. `-q` / `--quiet` drops the notes and the progress output
on stderr; errors are always reported.

## Output formats

The global `--format` / `-f` flag chooses the shape of stdout:

| Format | What stdout carries |
| ------ | ------------------- |
| `text` | The default: for people, aligned, coloured on a terminal. Its wording may change between versions; don't parse it. |
| `json` | One JSON value per command, on one line. |
| `jsonl` | One JSON value per line: a list command prints one line per item (`device list`), a stream one line per record as it happens (`power watch`, `device log --follow`, `device counters --watch`). |

Commands that stream until Ctrl-C refuse `--format json` (there would be no one value to print): use `jsonl`.

```bash
# one reading, fed to jq
dg power meter -f json | jq .current_ma

# a stream, one record per line, appended to a log
dg power watch --on-change -f jsonl >> power.log
```

The JSON shape of every command is part of the contract: a field is not removed or renamed without a CHANGELOG
entry. New fields can appear, so read the fields you need and ignore the rest.

## Colour

`text` on stdout is coloured when stdout is a terminal, and plain when it is piped or redirected, with `NO_COLOR`
set (to anything non-empty) or `TERM=dumb`. `--color always` or `--color never` override that. The plain text is
the same as the coloured one without the escape codes. `json` and `jsonl` are never coloured.

## Exit codes

| Code | Name           | Meaning |
| ---- | -------------- | ------- |
| 0    | `success`      | The command did what was asked. |
| 1    | `failure`      | No more specific category applies: the link to the dongle broke, a file couldn't be read, or the command ran but the result isn't what was asked (VBUS tripped instead of turning on, a memtest mismatch). |
| 2    | `bad_args`     | Wrong or invalid arguments the argument parser catches (a usage error). A value a command checks itself, such as `settings set plug_vbus_at_boot auto` without `--trip`, is still `1` for now. |
| 3    | `no_device`    | No dongle matched the selector (`--device`), or more than one did and none was chosen. |
| 4    | `device_error` | The dongle answered with an error, its firmware doesn't implement what was asked (update it), or it speaks an API this CLI can't talk to. |
| 5    | `timeout`      | The dongle didn't answer in time. |

The numbers and names don't change; a new category gets the next free number. A command reading a state back
(`power on`, `usb connect`) exits `1` when the state isn't the one asked for, even though no error was raised:
check the exit code, not only the output.

## Errors

Under `--format text` a failed command prints one line on stderr:

```text
donguru: error: no matching device: none connected
```

Under `--format json` and `jsonl` the same error is one JSON object on one line, also on stderr (stdout stays data
only, so a `jsonl` stream that fails halfway has only its records on stdout):

```json
{"error":{"exit":"device_error","code":4,"message":"power meter: ...","kind":"refused"}}
```

| Field | Meaning |
| ----- | ------- |
| `exit` | The exit code's name from the table above. |
| `code` | The exit code itself, the same number the process exits with. |
| `message` | What went wrong, with its causes, for a person to read. Its wording may change. |
| `kind` | Only when the dongle itself answered with an error: `unsupported`, `invalid_argument`, `in_use`, `busy`, `wrong_state`, `no_power`, `refused`, `timeout`, `fault`, `specific`, `other`; `unimplemented` when its firmware lacks the item, `incompatible` when its API is one this CLI can't talk to. |

Usage errors (exit code 2) are printed by the argument parser before the command runs, as text, whatever the
format.

```bash
if ! out=$(dg power on -f json 2>err.json); then
    jq -r .error.exit err.json   # e.g. no_device
fi
```
