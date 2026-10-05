# Donguru features and roadmap

This file is the single source of truth for what Donguru's firmware, host library and CLI do, what is broken and
what is planned, for humans and AI agents alike. [CHANGELOG.md](CHANGELOG.md) records what changed and when; the
[design docs](docs/design/index.md) say how things should work; this file records the current state.

Last full review: 3 Oct 2026 (commit `c702f0d`, 0.1.0). Built from the code, the design docs, `developer-notes.md`,
`bedrock_fw.json` and the B153A bring-up notes in tpm (P2534).

## How to use this file

- **Status** of each item:
  - ✅ done (for hardware: tested on a board, the note says which revision)
  - 🚧 in progress or partially done (the note says what is missing)
  - 🧪 implemented, builds, not tested on hardware yet
  - 🐛 implemented, but with known bugs
  - ⬜ stub: command, types or API exist but do nothing yet
  - 📋 planned
  - 💡 idea, not committed to
  - ⛔ blocked (the note says on what)
  - 🔍 probably done or obsolete, needs a check before closing
- **IDs** (`CLI-3`, `FW-5`) are stable: never renumber or reuse one. New items take the next free number of their
  area. Use the ID in commit messages, CHANGELOG entries and code `TODO`s (`// TODO(FW-8): ...`).
- **Target**: `revB` marks what testers need on the rev B dongles (end of 2026), `v1.0` what the CrowdSupply
  launch needs. No tag means not scheduled.
- Items are grouped by area. Each area lists what works first, then open items by priority.
- When finishing work, update the item in the same commit: mark it ✅ (or 🧪), add a pointer (module, command, API
  method) and move it up to the done items of its area. Don't delete done items. Items that turn out obsolete go to
  [Dropped and superseded](#dropped-and-superseded) with a one-line reason.
- A bug you find but don't fix gets an entry (🐛 on the feature, or a new item) with what triggers it.
- Small code-level gaps stay as `TODO` comments; only those that limit users or block a feature get an item.

## Firmware (`FW`)

- 🧪 **FW-1 Firmware base**: embassy on STM32G0B1CE, 64 MHz from HSI16 + PLL, USB clock from HSI48 trimmed by CRS,
  embedded_bedrock template 0.5.0 (`bedrock_fw.json`), defmt over RTT, `cnt` RAM and backup-register counters,
  fault handlers. 2K config page reserved at the end of flash (`memory.x`), not used yet.
- ✅ **FW-2 LEDs**: power and status LEDs on PF0 / PF1 with on / off / blink / breath animations and perceptual
  brightness (`src/led.rs`). B153A.
- ✅ **FW-3 ADC sampling**: continuous ADC1 into a DMA ring buffer with hardware oversampling and calibration;
  VDDA, die temperature, input voltage, input current, button / switch input, optionally P0 / P2 / P3
  (`src/adc.rs`, `adc::latest()`). B153A. Current sign: positive = charging the DUT (computer to DUT), negative =
  the DUT feeds the computer (0.1.1, from the schematic; check on the bench).
- ✅ **FW-4 Button and boot switch**: decoded from one ADC input (PA7) with debounce, edges and press duration
  (`src/button.rs`). B153A.
- ✅ **FW-5 USB hub bring-up**: hub works after I2C init (B153A, 29 Sep 2026; `examples/usb_serial.rs`). PF2 is
  NRST on this MCU, hub reset moved to PC13 for rev B.
- 🐛 **FW-6 Hub configuration**: hub defaults read back as 0, and a reMarkable 2 behind the hub gets descriptor
  error -32 (B153A notes, `hw/b153_typec_serial/report/assy_b153a.md`).
  Likely cause: D+/D- are crossed between the hub and the plug on port 2 (and port 4, SBU USB on Plus), so a
  full-speed device looks low-speed. Set the hub's port swap for ports 2 and 4 (`hw/b153_typec_serial/doc/hw_b153a.md`).
- 📋 **FW-7 USART on SBU with automatic TX/RX flip**: bridge to the host (CDC-ACM or over WireWeaver), flip by
  cable orientation. ^revB
- 📋 **FW-8 Power switch and USB data switches** per downstream port, with safe defaults at boot. ^revB
- 📋 **FW-9 Power meter calibration**: offsets and gain per board, stored in the config page. ^v1.0
- 📋 **FW-10 GPIO and I2C** on the external pins. ^v1.0
- 📋 **FW-11 `fw_info` / `hw_info`** (embedded_bedrock device contract): firmware name, version, git SHA, build
  time; board revision and serial. ^revB
- 📋 **FW-12 Firmware update without a probe**: STM32 system bootloader via the boot switch first, then a custom
  bootloader; the CLI side is CLI-10. ^v1.0
- 📋 **FW-13 HardFault handling for production**: reset instead of looping (TODO in `HardFault`); the backup-register
  counter already records the fault.

## API (`API`)

- 📋 **FW-14 Switched VBUS on the header** (J101 pin 10, PD1 `5V_EN`, net to be renamed VBUS_SW): it is the computer's
  VBUS as it is, 5 V or up to 20 V after PD. Off at boot; report the receptacle voltage (PA3) with the state; turn it
  off when VBUS changes unless the user allowed a higher voltage. ^revB
- 🧪 **API-1 WireWeaver over USB**: `DonguruApi` served by `fw/donguru/src/ww.rs` (USB FS, VID:PID `c0de:cafe`, API
  id in a string descriptor for `ww list`). Built against the local wire_weaver checkout; not yet tested with
  `ww list` on hardware.
- ✅ **API-2 `led_on` / `led_off`**: status LED only, the template's example methods.
- 📋 **API-3 Full API**: ports (power, data), power meter readings and streams, button / switch events, USART bridge,
  GPIO, I2C, device info; designed together with the CLI command tree. ^revB

## Host library (`CORE`)

- ⬜ **CORE-1 Device enumeration and selection**: `list()` is `todo!()`, `select` resolves nothing yet
  (`donguru-core/src/device.rs`). Selectors: serial, index, USB path; auto-select when exactly one dongle is present.
  ^revB
- ✅ **CORE-2 Error type**: `thiserror` errors mapped to exit codes by the CLI (`donguru-core/src/error.rs`).
- 📋 **CORE-3 Transport trait with a mock**: so commands can be tested without hardware (design doc, *Transport
  model*).
- 📋 **CORE-4 Python bindings** over the Rust core. ^v1.0

## CLI (`CLI`)

- ✅ **CLI-1 Command tree and global flags**: `info`, `device`, `usb`, `gpio`, `udev`, `config`, `completions`;
  `--device`, `--format text|json|jsonl`, `--color`, `-v` / `-q` (`donguru-cli/src/cli.rs`).
- ✅ **CLI-2 Colour theme** adapted to terminal capabilities (`src/theme.rs`, `examples/theme_preview.rs`).
- ✅ **CLI-3 Shell completions** (`donguru completions <shell>`).
- ⬜ **CLI-4 External subcommands**: clap captures unknown subcommands, but `src/commands/external.rs` only prints
  "not implemented": resolve `donguru-<name>` on `PATH` (with a near-match hint), export the resolved device and
  format (the doc comment says `DONGURU_CTX_*`, the design doc `DONGURU_DEVICE` / `DONGURU_OUTPUT`: pick one), exec
  it, and list plugins in `donguru help`.
- ⬜ **CLI-5 `gpio read / write / configure / watch`**: arguments parsed, prints "not implemented". Needs FW-10,
  API-3, CORE-1.
- ⬜ **CLI-6 `usb list / attach / detach / power / data`**: stub. Needs FW-8, API-3. ^revB
- ⬜ **CLI-7 `device list / info / select`, `info`**: stub. Needs CORE-1. ^revB
- 🚧 **CLI-8 `udev`**: `generate` writes the rule (uaccess / group / all); `install`, `uninstall`, `status` are stubs.
- ⬜ **CLI-9 `config generate / show / path / validate`**: stub. Layering per the design doc (defaults, system,
  user, project file, `DONGURU_*`, flags).
- 📋 **CLI-10 `fw version` / `fw update`**: in the design doc, not in the command tree yet. Needs FW-11, FW-12.
- 📋 **CLI-11 `power on / off / status / meter / watch`**: in the design doc, not in the command tree yet. ^revB
- 📋 **CLI-16 Header VBUS output with a clear warning**: enabling the header's switched VBUS (FW-14) shows the present
  voltage and a warning that it is the computer's VBUS, not a regulated 5 V; above 5.5 V it needs an explicit
  confirmation flag. Same warning in the GUI, Python and docs. ^revB
- 📋 **CLI-12 `i2c scan / read / write` and `uart`**: in the design doc, not in the command tree yet.
- 🐛 **CLI-13 Exit codes differ from the notes**: `src/exit.rs` has device error = 4, timeout = 5 and no transport
  code; `developer-notes.md` has transport = 4, device = 5, timeout = 6. Decide before anything ships.
- 📋 **CLI-14 `--version` with git SHA and build time** (`build.rs`), per the version rule in AGENTS.md.
- 📋 **CLI-15 Output renderer**: commands return typed values, one renderer for text / json / jsonl
  (`developer-notes.md`).

## Packaging and docs (`PKG`)

- ✅ **PKG-1 Documentation site**: Zensical, quickstart / usage / scripting / design pages, published by
  `.github/workflows/docs.yml`. User pages are mostly placeholders until commands work.
- ✅ **PKG-2 CI and hooks**: `just verify` in GitHub Actions, prek pre-commit / pre-push hooks.
- 📋 **PKG-3 Packages**: `.deb` / `.rpm` first, then Homebrew and Chocolatey; man pages, completions and the udev rule
  installed by the package (`docs/design/packaging/`). ^v1.0
- 📋 **PKG-4 License**: the workspace says `license = "TBD"`. ^v1.0

## Dropped and superseded

(none yet)
