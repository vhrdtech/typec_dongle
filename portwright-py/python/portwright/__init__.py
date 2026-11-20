"""Drive a Portwright USB Type-C dongle: a blocking API with SI values as floats (V, A, W, s, C, J).

    import portwright
    pw = portwright.open()            # the only dongle, or by label, serial, index or USB path
    pw.power.on()
    for r in pw.meter.watch(interval=0.5, count=10):
        print(r.voltage, r.current)

`portwright.mock.MockDevice` runs a dongle in this process for tests without hardware.
"""

from ._portwright import (
    Deadband,
    DeviceError,
    DeviceInfo,
    Dongle,
    Error,
    Header,
    Incompatible,
    Info,
    Leds,
    LinkError,
    Meter,
    MeterWatch,
    NoDevice,
    Power,
    Reading,
    Settings,
    Timeout,
    Totals,
    UartConfig,
    UartPort,
    Uarts,
    Unimplemented,
    Usb,
    VbusMode,
    VbusStatus,
    __version__,
    list,
    open,
)

__all__ = [
    "Deadband",
    "DeviceError",
    "DeviceInfo",
    "Dongle",
    "Error",
    "Header",
    "Incompatible",
    "Info",
    "Leds",
    "LinkError",
    "Meter",
    "MeterWatch",
    "NoDevice",
    "Power",
    "Reading",
    "Settings",
    "Timeout",
    "Totals",
    "UartConfig",
    "UartPort",
    "Uarts",
    "Unimplemented",
    "Usb",
    "VbusMode",
    "VbusStatus",
    "__version__",
    "list",
    "open",
]
