---
icon: lucide/power
---

# Power and data switches

Donguru switches the power and the USB data going to the device under test (DUT), and the switched VBUS on the
header. The firmware keeps the limits, so a script or a host that hangs can't leave a DUT on a voltage it didn't
ask for.

| API | What it switches | Hardware |
|---|---|---|
| `plug.vbus_mode`, `plug.vbus` | VBUS from the computer to the plug (the DUT) | U601 load switch (AOZ1375) |
| `plug.vbus_cycle(off, release_sbu)` | plug VBUS off for `off`, then back | the same |
| `plug.data` | USB data (D+/D-) to the DUT: `Connected` / `Cut` | U701 |
| `header.vbus_sw_mode`, `header.vbus_sw` | header pin 10 (VBUS_SW) | Q102 |
| `header.relay` | the isolated photo-MOS contact between header pins 11 and 12 | U101 |

After a reset everything is off: both VBUS switches `ForceOff`, data connected, relay open. The board's pull-downs
give the same state while the MCU is in reset or halted.

## VBUS modes

Each VBUS switch has a mode (`plug.vbus_mode`, `header.vbus_sw_mode`) and a state (`plug.vbus`, `header.vbus_sw`).

- **`ForceOff`**: off.
- **`ForceOn { max_voltage }`**: on while the computer's VBUS stays at or under `max_voltage`. Hosts use 5.5 V
  unless you ask for more.
- **`Auto { max_voltage, max_current, max_power }`**: on, and cut when any of the three is exceeded. The current
  and power are checked in both directions (DUT drawing or feeding back). Plug only: the header's VBUS_SW is
  taken off before the current shunt, so its current isn't measured and `Auto` there is `InvalidArgument`.

The limits apply to the VBUS coming from the computer (the receptacle), as `meter.reading` shows it: when a USB-PD
charger renegotiates from 5 V to 20 V, a switch with `max_voltage` 5.5 V goes off.

The state is `Off`, `On`, `Tripped(OverVoltage | OverCurrent | OverPower)` when the firmware cut it, or `Fault`
when the plug switch (U601) latched off on its own (short circuit, over-temperature, its 24 V over-voltage).

- **Setting a guarded mode while VBUS is already over `max_voltage` is refused** (`Refused`), and nothing changes.
- **A trip or a fault stays latched**: the switch remains off when the voltage comes back down. Writing a mode
  again (any mode) re-arms it; for a fault that also toggles U601's enable, which is what releases its latch.
- A limit that isn't a positive number, or a mode the firmware doesn't know (from a newer host), is
  `InvalidArgument`.

!!! warning "A software trip is not a fuse"

    The firmware checks the limits on every ADC update, about every 15 ms, against the worst voltage and current
    seen since the previous update. Cutting takes up to one update: fine for a PD renegotiation or a DUT that draws
    too much, too slow to protect against a short. The computer-to-DUT direction has no hardware current limit
    (U601 only limits DUT-to-computer, and protects against a dead short).

## Power cycle

`plug.vbus_cycle(off, release_sbu)` turns plug VBUS off for `off` (0 to 60 s), then returns to the mode through the
same check as a mode write. It is answered when VBUS is back on, with the state, or with `OverVoltage { measured }`
when VBUS is over the limit by then (the switch is `Tripped(OverVoltage)`). With `release_sbu = true` the SBU lines
go to Hi-Z first, so nothing on SBU powers the DUT while it is off.

- In `ForceOff` there is nothing to return to: `WrongState`.
- A second cycle while one runs is `Refused`. Writing the mode during a cycle ends it: the write stands.

## Watching

Every property above can be observed: a trip, a fault, the end of a cycle and each write are pushed to the hosts
that observe them.
