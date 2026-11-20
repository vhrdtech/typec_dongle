# Types of the extension module (donguru-py/src): keep in step with the #[pymethods] there.

import builtins
from types import TracebackType
from typing import Literal

__version__: str

def list() -> builtins.list[DeviceInfo]: ...
def open(selector: int | str | None = None) -> Dongle: ...

class Error(Exception): ...
class NoDevice(Error): ...

class DeviceError(Error):
    kind: str
    """`"unsupported"`, `"invalid_argument"`, `"in_use"`, `"busy"`, `"wrong_state"`, `"no_power"`, `"refused"`,
    `"timeout"`, `"fault"`, `"specific"` or `"other"`."""
    item: str

class Unimplemented(Error): ...
class Timeout(Error): ...
class Incompatible(Error): ...
class LinkError(Error): ...

class DeviceInfo:
    index: int
    serial: str
    path: str
    label: str
    api_version: str | None

class Info:
    variant: str
    firmware: str | None
    board: str | None
    uid: str | None
    label: str | None
    last_reset: str | None

class Reading:
    voltage: float
    current: float
    power: float
    t: float | None

class Totals:
    charge: float
    energy: float
    peak_current: float
    since: float
    @property
    def charge_mah(self) -> float: ...
    @property
    def energy_wh(self) -> float: ...

class VbusStatus:
    state: Literal["off", "on", "fault", "tripped", "unknown"]
    trip_reason: Literal["over_voltage", "over_current", "over_power", "unknown"] | None
    @property
    def on(self) -> bool: ...
    @property
    def tripped(self) -> bool: ...

class VbusMode:
    @staticmethod
    def off() -> VbusMode: ...
    @staticmethod
    def on(max_voltage: float = 5.5) -> VbusMode: ...
    @staticmethod
    def auto(max_current: float, max_voltage: float = 5.5) -> VbusMode: ...
    @property
    def mode(self) -> Literal["off", "on", "auto", "unknown"]: ...
    @property
    def max_voltage(self) -> float | None: ...
    @property
    def max_current(self) -> float | None: ...
    @property
    def max_power(self) -> float | None: ...

class Deadband:
    voltage: float
    current: float
    def __init__(self, voltage: float, current: float) -> None: ...

Parity = Literal["none", "odd", "even"]
Flow = Literal["none", "hardware", "software", "high_z", "test"]
ButtonAction = Literal["off", "toggle_vbus", "cycle_vbus", "toggle_data"]

class UartConfig:
    baud: int
    data_bits: int
    parity: Parity
    stop_bits: int
    flow: Flow
    prevent_back_feed: bool
    io_level: float | None
    idle_chars: int
    def __init__(
        self,
        baud: int = 115200,
        data_bits: int = 8,
        parity: Parity = "none",
        stop_bits: int = 1,
        flow: Flow = "none",
        prevent_back_feed: bool = False,
        io_level: float | None = None,
        idle_chars: int = ...,
    ) -> None: ...

class Power:
    def on(self, max_voltage: float = 5.5, trip: float | None = None) -> VbusStatus: ...
    def off(self) -> VbusStatus: ...
    def cycle(self, off: float = 0.5, release_sbu: bool = False) -> VbusStatus: ...
    @property
    def state(self) -> VbusStatus: ...
    mode: VbusMode

class MeterWatch:
    def __iter__(self) -> MeterWatch: ...
    def __next__(self) -> Reading: ...
    def close(self) -> None: ...
    def __enter__(self) -> MeterWatch: ...
    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> bool: ...

class Meter:
    def read(self) -> Reading: ...
    def watch(
        self,
        interval: float = 0.5,
        count: int | None = None,
        on_change: bool = False,
        samples: bool = False,
    ) -> MeterWatch: ...
    def totals(self) -> Totals: ...
    def reset_totals(self) -> None: ...
    def temperature(self) -> float: ...
    window: float
    deadband: Deadband

class Usb:
    connected: bool
    def connect(self) -> bool: ...
    def disconnect(self) -> bool: ...

class Header:
    relay: bool
    @property
    def vbus(self) -> VbusStatus: ...
    vbus_mode: VbusMode
    def vbus_on(self, max_voltage: float = 5.5, trip: float | None = None) -> VbusStatus: ...
    def vbus_off(self) -> VbusStatus: ...

class UartPort:
    def configure(
        self,
        baud: int = 115200,
        data_bits: int = 8,
        parity: Parity = "none",
        stop_bits: int = 1,
        flow: Flow = "none",
        prevent_back_feed: bool = False,
        io_level: float | None = None,
        idle_chars: int = ...,
    ) -> UartConfig: ...
    def apply(self, config: UartConfig) -> UartConfig: ...

class Uarts:
    @property
    def plug(self) -> UartPort: ...
    @property
    def header(self) -> UartPort: ...

class Settings:
    plug_vbus_at_boot: VbusMode
    button_action: ButtonAction
    meter_window: float
    meter_deadband: Deadband
    uart_plug: UartConfig
    def reset(self) -> None: ...

class Leds:
    def identify(self, seconds: float = 5.0) -> None: ...

class Dongle:
    @property
    def entry(self) -> DeviceInfo: ...
    @property
    def serial(self) -> str: ...
    def info(self) -> Info: ...
    @property
    def firmware_version(self) -> str | None: ...
    label: str
    @property
    def power(self) -> Power: ...
    @property
    def meter(self) -> Meter: ...
    @property
    def usb(self) -> Usb: ...
    @property
    def header(self) -> Header: ...
    @property
    def uart(self) -> Uarts: ...
    @property
    def settings(self) -> Settings: ...
    @property
    def leds(self) -> Leds: ...
    def close(self) -> None: ...
    def __enter__(self) -> Dongle: ...
    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> bool: ...

class MockDevice:
    def __init__(self, path: str | None = None) -> None: ...
    def open(self) -> Dongle: ...
    def set_reading(self, voltage: float, current: float) -> None: ...
    def set_totals(self, charge: float, energy: float, peak_current: float, since: float) -> None: ...
    def set_settings_fault(self, code: int | None = None) -> None: ...
    @property
    def plug_vbus_mode(self) -> VbusMode: ...
    @property
    def vbus_cycles(self) -> int: ...
    @property
    def data_connected(self) -> bool: ...
    @property
    def relay(self) -> bool: ...
    @property
    def identified(self) -> int: ...
