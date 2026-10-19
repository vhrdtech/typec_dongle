---
icon: lucide/terminal
---

# Usage

A task-oriented guide to everyday commands. Most of the command tree is still being built (see
[FEATURES.md](https://github.com/vhrdtech/typec_dongle/blob/main/FEATURES.md) for current status); this page grows
with it. `dg` is a short alias for `donguru` and takes the same arguments.

## Power to the device under test

`donguru power` drives the plug's VBUS switch (U601) and reads the power meter; `donguru usb` is the USB data
switch next to it. See [Power and data switches](power.md) for the modes and the guard the firmware keeps.

```bash
# plug VBUS, header VBUS_SW and the meter in one table
dg power status

# turn the DUT on (ForceOn at the 5.5 V default), and off again
dg power on
dg power off

# a PD charger needs a higher limit, confirmed explicitly; --trip switches to Auto with a current limit too
dg power on --allow-voltage 20V
dg power on --allow-voltage 20V --trip 2A

# off for 500 ms, SBU to Hi-Z first so nothing back-powers the DUT, then back to the mode
dg power cycle --off 500ms --release-sbu

# one reading, or stream it until Ctrl-C
dg power meter
dg power watch --interval 100ms
dg power watch --on-change -f jsonl >> power.log
# every ADC sequence (~1.6 kHz, not averaged), each with the dongle's own time: a current trace of the DUT
dg power watch --samples -f jsonl > trace.jsonl

# charge and energy since boot, zeroed with --reset
dg power totals
dg power totals --reset

# cut (or restore) the DUT's USB data lines without touching power
dg usb status
dg usb disconnect
dg usb connect
```

`on` / `off` / `cycle` exit non-zero unless VBUS actually ends up where asked: a trip or a latched fault is a
scriptable failure, not just a successful call.

## The header's switched VBUS and relay

`donguru header` controls J101's own switched VBUS (Q102) and the isolated relay (U101). VBUS_SW carries the
computer's own VBUS, not a regulated 5 V supply, so `header vbus on` always shows the present voltage and a
warning; going above the 5.5 V default needs the same `--allow-voltage` confirmation as `power on`.

```bash
dg header vbus status
dg header vbus on
dg header vbus on --allow-voltage 9V   # confirms the higher limit
dg header vbus off

dg header relay status
dg header relay on
dg header relay off
```
