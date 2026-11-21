---
icon: lucide/terminal
---

# Usage

A task-oriented guide to everyday commands. Most of the command tree is still being built (see
[FEATURES.md](https://github.com/vhrdtech/portwright/blob/main/FEATURES.md) for current status); this page grows
with it. `pw` is a short alias for `portwright` and takes the same arguments.

## Power to the device under test

`portwright power` drives the plug's VBUS switch (U601) and reads the power meter; `portwright usb` is the USB data
switch next to it. See [Power and data switches](power.md) for the modes and the guard the firmware keeps.

```bash
# plug VBUS, header VBUS_SW and the meter in one table
pw power status

# turn the DUT on (ForceOn at the 5.5 V default), and off again
pw power on
pw power off

# a PD charger needs a higher limit, confirmed explicitly; --trip switches to Auto with a current limit too
pw power on --allow-voltage 20V
pw power on --allow-voltage 20V --trip 2A

# off for 500 ms, SBU to Hi-Z first so nothing back-powers the DUT, then back to the mode
pw power cycle --off 500ms --release-sbu

# one reading, or stream it until Ctrl-C
pw power meter
pw power watch --interval 100ms
pw power watch --on-change -f jsonl >> power.log
# every ADC sequence (~1.6 kHz, not averaged), each with the dongle's own time: a current trace of the DUT
pw power watch --samples -f jsonl > trace.jsonl

# charge and energy since boot, zeroed with --reset
pw power totals
pw power totals --reset

# cut (or restore) the DUT's USB data lines without touching power
pw usb status
pw usb disconnect
pw usb connect
```

`on` / `off` / `cycle` exit non-zero unless VBUS actually ends up where asked: a trip or a latched fault is a
scriptable failure, not just a successful call.

## The header's switched VBUS and relay

`portwright header` controls J101's own switched VBUS (Q102) and the isolated relay (U101). VBUS_SW carries the
computer's own VBUS, not a regulated 5 V supply, so `header vbus on` always shows the present voltage and a
warning; going above the 5.5 V default needs the same `--allow-voltage` confirmation as `power on`.

```bash
pw header vbus status
pw header vbus on
pw header vbus on --allow-voltage 9V   # confirms the higher limit
pw header vbus off

pw header relay status
pw header relay on
pw header relay off
```

## Bring-up and debug

`portwright debug` sets MCU pins one by one by net name, compares the pins at boot with the reset state, and reads
the hub's and UCPD's registers: see [Bring-up and debug](debug.md).

```bash
pw debug pin set RELAY_EN high
pw debug pins --boot --diff
pw debug hub read 0x00 4
pw debug ucpd
```
