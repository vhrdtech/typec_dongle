---
icon: lucide/file-code
---

# Python

The `portwright` package drives a dongle from Python: power, the meter, USB data, the header, the UARTs and the
settings. It is the same Rust library the `pw` CLI uses, built as a Python extension (PyO3), so both behave the same
way and report the same errors.

The API is blocking: each call returns when the dongle has answered. Values are plain floats in SI units: volts,
amperes, watts, seconds, coulombs, joules. Choices are strings (`"on"`, `"toggle_vbus"`, `"even"`).

```python
import portwright

with portwright.open() as pw:                  # the only dongle connected
    pw.power.on(trip=1.5)                   # VBUS to the DUT, cut above 1.5 A
    for r in pw.meter.watch(interval=0.5, count=10):
        print(f"{r.voltage:.3f} V  {r.current:.3f} A  {r.power:.2f} W")
    print(pw.meter.totals().charge_mah, "mAh")
    pw.power.off()
```

## Install

The package isn't on PyPI yet. Build it from a checkout of the repository with [uv](https://docs.astral.sh/uv/)
(it needs a Rust toolchain; maturin is fetched by uv):

```sh
uv add --editable path/to/typec_dongle/portwright-py   # into your uv project
just py-build                                        # or a wheel into target/dist
```

One wheel works for every CPython from 3.10 on. It reaches the dongle the same way the CLI does: when `pw list`
sees it, so does `portwright.list()`.

## Finding a dongle

`portwright.list()` lists the connected dongles without opening them: `index`, `serial`, `path` (USB port),
`label` and `api_version` of each.

`portwright.open(selector)` opens one. Without a selector it opens the only dongle connected and raises `NoDevice` when
there are none or several. A selector picks like `pw --device`:

| Selector | Picks |
|---|---|
| `0`, `"index:0"` | the first in `list()` |
| `"0049002A"`, `"serial:0049002A"` | by serial, or a unique start of it |
| `"3-1.2"`, `"path:3-1.2"` | by USB port |
| `"bench-1"`, `"label:bench-1"` | by user label (any case), when no serial matches |

The label is kept on the dongle: `pw.label = "bench-1"`.

A `Dongle` closes with `pw.close()`, at the end of a `with` block, or when it is garbage-collected. `pw.info()`
reads variant, firmware, board, UID, label and the cause of the last reset at once; `pw.firmware_version` only the
firmware (`"portwright 0.20.27"`).

## Power

`pw.power` is the plug VBUS to the DUT (see [Power and data switches](power.md)).

```python
pw.power.on()                        # ForceOn, cut above 5.5 V
pw.power.on(max_voltage=5.25, trip=0.5)   # Auto: also cut above 0.5 A
status = pw.power.cycle(off=1.0)     # off for 1 s, then back to the mode it was in
pw.power.off()
```

Each of these returns a `VbusStatus`: `state` is `"off"`, `"on"`, `"fault"` or `"tripped"`, with `trip_reason`
(`"over_voltage"`, `"over_current"`, `"over_power"`) when the firmware cut it. A trip is a state, not an
exception: check `status.on` or `status.tripped`. `pw.power.state` reads it now; `pw.power.mode` reads or sets the
mode as a `VbusMode` (`VbusMode.off()`, `VbusMode.on(max_voltage)`, `VbusMode.auto(max_current, max_voltage)`).

`pw.header.vbus_on()`, `vbus_off()`, `vbus`, `vbus_mode` do the same for the switched VBUS on the header, and
`pw.header.relay = True` closes the isolated relay contact.

## USB data

```python
pw.usb.disconnect()      # cut D+/D- to the DUT
pw.usb.connect()
pw.usb.connected         # True / False, also settable
```

## Meter

```python
r = pw.meter.read()                  # one reading: r.voltage, r.current, r.power
pw.meter.temperature()               # board temperature in °C
t = pw.meter.totals()                # t.charge (C), t.energy (J), t.peak_current, t.since (s)
t.charge_mah, t.energy_wh
pw.meter.reset_totals()
pw.meter.window = 0.1                # average a reading over 100 ms (until reset)
```

`pw.meter.watch()` streams readings as an iterator:

- `watch(interval=0.5)`: one reading every 0.5 s,
- `watch(on_change=True)`: only when the reading moves beyond `pw.meter.deadband`, at most one per `interval`,
- `watch(samples=True)`: every ADC sample (about 1.6 kHz), each with the device time `r.t` in s.

`count=N` stops after N readings, otherwise it runs until you stop it: `break`, `close()`, or a `with` block.
Ctrl-C works while it waits.

```python
# peak current over about 10 s of samples
peak = max(r.current for r in pw.meter.watch(samples=True, count=16000))
```

## UART

`pw.uart.plug` is the UART on the SBU lines of the plug, `pw.uart.header` the one on the header (IO7/IO8).
`configure()` sets the line up and returns what was set, with the baud rate the divider settled on:

```python
cfg = pw.uart.plug.configure(baud=115200, parity="none", stop_bits=1, io_level=3.3)
print(cfg.baud)
```

The same settings as a value: `portwright.UartConfig(baud=..., ...)`, applied with `apply(cfg)`.

## Settings

Settings kept in flash, applied at once where they have a live counterpart:

```python
s = pw.settings
s.plug_vbus_at_boot = portwright.VbusMode.off()
s.button_action = "toggle_vbus"      # "off", "toggle_vbus", "cycle_vbus", "toggle_data"
s.meter_window = 0.05
s.meter_deadband = portwright.Deadband(voltage=0.05, current=0.01)
s.uart_plug = portwright.UartConfig(baud=1_000_000)
s.reset()                            # every setting and the label back to defaults
```

`pw.leds.identify(5.0)` blinks the LEDs for 5 s to find a dongle on the bench.

## Errors

Every error is a `portwright.Error`; catch the category you can handle:

| Exception | When |
|---|---|
| `NoDevice` | no dongle matched the selector, or more than one |
| `DeviceError` | the dongle refused the call: `.kind` (`"wrong_state"`, `"in_use"`, `"fault"`, ...) and `.item` |
| `Unimplemented` | the firmware doesn't have this yet: update it |
| `Timeout` | the dongle didn't answer in time |
| `Incompatible` | the firmware speaks an API this package can't talk to |
| `LinkError` | USB failed, the dongle was unplugged, or it was closed |

Values out of range (a NaN, a negative interval, an unknown parity) raise `ValueError` before anything is sent.

## Testing scripts without a dongle

`portwright.mock.MockDevice` runs a dongle inside the process. It follows the firmware's rules (VBUS modes, settings,
errors) and its meter shows what the test sets:

```python
from portwright.mock import MockDevice

def test_my_script():
    mock = MockDevice()
    mock.set_reading(voltage=5.02, current=0.25)
    pw = mock.open()
    pw.power.on(trip=1.0)
    assert pw.meter.read().current == 0.25
    assert mock.plug_vbus_mode.mode == "auto"
```

The package ships type hints, so editors complete the levels and their arguments.
