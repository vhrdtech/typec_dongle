---
icon: lucide/wrench
---

# Development

This page describes how to set up a local development environment.

## Prerequisites

* [uv](https://docs.astral.sh/uv/)
* [rustup](https://rustup.rs)
* [just](https://github.com/casey/just)
* [prek](https://github.com/j178/prek)

## Git hooks

Code quality checks run before commit(s) and/or pushes using [prek](https://github.com/j178/prek).
For details see `.pre-commit-config.yaml`.

Set the hooks up once after cloning:

```bash
just hooks-install
```

## Just Tasks
Generally you find automations or entry points for automated tasks in the `justfile`.
This should and shall be the single source of truth for triggering tasks and actions.

In order to get a list of all available taks just run:

`just` or `just --list`

## Working on the documentation

The documentation site is built with [Zensical](../reference/zensical.md).
Common tasks are exposed through the `justfile`:

```bash
# Build the static site into ./site
just build-docs

# Serve the docs locally with live reload
just serve-docs
```

See the [Reference](../reference/zensical.md) section for details on
authoring content.

## Hardware: testing SWD-host (FW-15)

The dongle's firmware now hosts SWD over the plug SBU lines (CMSIS-DAP v2, `fw/donguru/src/swd.rs` and
`src/cmsis_dap.rs`): built and `cargo build`-checked, **not yet run on hardware**. This is the checklist for the
first bring-up, once a board is on the bench. Target: a B129 CANnify Micro (STM32G0B1CET, same family as
Donguru's own MCU) wired through a USB-C breakout to Donguru's plug, as the DUT for the P2625 chunky-ota
power-cut bench.

### Wiring

Plug SBU1/SBU2 carry SWCLK/SWDIO (`hw/b153_typec_serial/doc/hw_b153a.md` §8). On a USB-C breakout, SBU1/SBU2 are
usually labelled `SBU1`/`SBU2` or `A8`/`B8`; there is no dedicated SWD connector, so wire them directly to the
CANnify Micro's SWCLK/SWDIO test points (plus GND, and VDD only if the DUT is not separately powered - check the
breakout and the CANnify Micro schematic before connecting). There is no NRESET/NTRST wire on this path: a
connect/program cycle relies on the target's own power-on or a software reset, not a hardware reset line.

### Checklist

1. **Continuity first, power off**: with a multimeter, confirm which USB-C breakout pins are SBU1/SBU2 before
   powering anything, and that they reach the intended CANnify Micro test points and nothing else (no short to
   VBUS/GND).
2. **Firmware builds and flashes**: `cd fw/donguru && cargo build && cargo build --release`, then flash via
   Donguru's own SWD (receptacle side, PA14/PA13, unrelated to this feature) or `cargo run` with a probe on the
   Donguru board itself.
3. **USB enumeration**: plug Donguru into the host, `ww list` should still show the WireWeaver interface; `lsusb -v`
   (Linux) or Device Manager (Windows) should show a second interface string containing `CMSIS-DAP`
   (`src/cmsis_dap.rs`'s `INTERFACE_STRING`).
4. **Orientation, normal first**: with the breakout in its first orientation, try the `probe-rs` command below. If
   it fails, flip the breakout (or send `plug.orientation = Swapped` once CLI-17 is wired up) and retry
   before assuming a wiring fault.
5. **Scope SWCLK/SWDIO**: confirm the waveform is clean at the default 400 kHz (`src/swd.rs::DEFAULT_FREQUENCY_HZ`)
   — no excessive ringing, correct levels (3.3 V, R211/R212 bypass the level shifter so OE must stay low, see the
   module doc) — before trusting a successful connect to mean the timing has real margin. Increase
   `MAX_FREQUENCY_HZ` only after measuring margin at a higher rate.
6. **`DAP_Disconnect` / Hi-Z**: after a `probe-rs` session, confirm with the scope or a multimeter that SWCLK and
   SWDIO float (no driven level) once probe-rs disconnects, and again after `swd_release` (once CLI-17 or `ww`
   raw calls can trigger it) — this is what the power-cut bench relies on.
7. **Power-cut bench dry run (P2625)**: with the DUT connected only through Donguru's switched VBUS (Q102) and
   SWD as above: start a `probe-rs` session, call `swd_release`, then cut VBUS, and confirm on the scope that
   SWCLK/SWDIO stay released (no back-powering the DUT through them) through the cut.
8. **Plug CC drive (FW-17, stopgap)**: with a multimeter or scope on the plug's CC1/CC2 (PD0/PD2), confirm both
   read about 3.2 V during a `probe-rs` session and float (no driven level) after `DAP_Disconnect` or
   `swd_release`. On a B129A DUT, also check TP1 `SWD_SEL` goes high while connected — that is what gates its
   SBU->SWD mux onto the plug.

### `probe-rs` command

Find the exact chip name first (the CANnify Micro is STM32G0B1CET, likely `STM32G0B1CETx` or similar in probe-rs's
database):

```bash
probe-rs chip list | grep -i G0B1
```

Then, with Donguru's CMSIS-DAP v2 interface enumerated (step 3 above) and a `DapLink`/generic CMSIS-DAP probe
selection:

```bash
probe-rs info --chip STM32G0B1CETx --probe <VID:PID or serial from `probe-rs list`>
```

`probe-rs list` should show Donguru's CMSIS-DAP interface (VID:PID `c0de:cafe` unless changed, see `src/ww.rs`) if
step 3 passed. `probe-rs info` is a good first smoke test: it only needs `DAP_Connect`, a few DP/AP reads and
`DAP_Disconnect`, so a successful run exercises most of `src/swd.rs` without touching the target's flash.
