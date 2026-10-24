"""The `donguru` package against donguru-core's in-process mock dongle (a B153A Main at boot)."""

import pytest

import donguru
from donguru.mock import MockDevice


@pytest.fixture
def mock():
    return MockDevice()


@pytest.fixture
def dg(mock):
    with mock.open() as dg:
        yield dg


def test_version():
    assert donguru.__version__.count(".") == 2


def test_info(dg):
    info = dg.info()
    assert info.variant == "Main"
    assert info.uid == "0049002A3133510B37363436"
    assert dg.entry.serial == dg.serial
    assert "Dongle(" in repr(dg)


def test_label(dg):
    assert dg.label == ""
    dg.label = "bench-1"
    assert dg.label == "bench-1"
    assert dg.info().label == "bench-1"


def test_power_on_off(dg, mock):
    assert dg.power.state.state == "off"
    status = dg.power.on()
    assert status.on and not status.tripped
    assert str(status) == "on"
    assert dg.power.mode.mode == "on"
    assert dg.power.mode.max_voltage == pytest.approx(5.5)
    assert mock.plug_vbus_mode.mode == "on"
    assert not dg.power.off().on
    assert mock.plug_vbus_mode == donguru.VbusMode.off()


def test_power_trip(dg, mock):
    dg.power.on(trip=1.5)
    mode = dg.power.mode
    assert mode.mode == "auto"
    assert mode.max_current == pytest.approx(1.5)
    assert mode.max_power == pytest.approx(1.5 * 5.5)
    assert repr(mode) == "VbusMode.auto(max_current=1.5, max_voltage=5.5)"


def test_power_cycle(dg, mock):
    dg.power.on()
    status = dg.power.cycle(off=0.05)
    assert status.on
    assert mock.vbus_cycles == 1


def test_power_mode_setter(dg):
    dg.power.mode = donguru.VbusMode.auto(max_current=0.5, max_voltage=5.25)
    assert dg.power.mode == donguru.VbusMode.auto(0.5, 5.25)


def test_bad_values(dg):
    with pytest.raises(ValueError):
        dg.power.on(max_voltage=float("nan"))
    with pytest.raises(ValueError):
        donguru.VbusMode.on(max_voltage=1e12)


def test_meter_read(dg, mock):
    r = dg.meter.read()
    assert r.voltage == pytest.approx(5.0)
    assert r.current == 0.0
    mock.set_reading(voltage=5.02, current=0.25)
    r = dg.meter.read()
    assert r.voltage == pytest.approx(5.02)
    assert r.current == pytest.approx(0.25)
    assert r.power == pytest.approx(5.02 * 0.25, rel=1e-3)


def test_meter_watch(dg, mock):
    mock.set_reading(voltage=5.0, current=0.1)
    readings = list(dg.meter.watch(interval=0.02, count=3))
    assert len(readings) == 3
    assert all(r.current == pytest.approx(0.1) for r in readings)


def test_meter_watch_close(dg):
    with dg.meter.watch(interval=0.02) as w:
        first = next(w)
        assert first.voltage == pytest.approx(5.0)
    with pytest.raises(StopIteration):
        next(w)


def test_meter_watch_flags(dg):
    with pytest.raises(ValueError):
        dg.meter.watch(on_change=True, samples=True)


def test_meter_totals(dg, mock):
    mock.set_totals(charge=3.6, energy=18.0, peak_current=1.2, since=60.0)
    t = dg.meter.totals()
    assert t.charge == pytest.approx(3.6)
    assert t.charge_mah == pytest.approx(1.0)
    assert t.energy_wh == pytest.approx(0.005)
    assert t.peak_current == pytest.approx(1.2)
    assert t.since == pytest.approx(60.0)
    dg.meter.reset_totals()
    assert dg.meter.totals().charge == 0.0


def test_meter_settings(dg):
    assert dg.meter.temperature() == pytest.approx(30.0)
    dg.meter.window = 0.1
    assert dg.meter.window == pytest.approx(0.1)
    dg.meter.deadband = donguru.Deadband(voltage=0.05, current=0.01)
    assert dg.meter.deadband == donguru.Deadband(0.05, 0.01)


def test_usb(dg, mock):
    assert dg.usb.connected
    dg.usb.disconnect()
    assert not dg.usb.connected
    assert not mock.data_connected
    dg.usb.connect()
    assert mock.data_connected
    dg.usb.connected = False
    assert not mock.data_connected


def test_header(dg, mock):
    assert not dg.header.relay
    dg.header.relay = True
    assert mock.relay
    assert dg.header.vbus.state == "off"
    assert dg.header.vbus_on().on
    assert dg.header.vbus_mode.mode == "on"
    assert not dg.header.vbus_off().on


def test_uart_configure(dg):
    c = dg.uart.header.configure(baud=1_000_000)
    assert c.baud == 1_000_000
    assert c.parity == "none"
    with pytest.raises(ValueError):
        dg.uart.header.configure(parity="sometimes")


def test_settings(dg):
    s = dg.settings
    assert s.plug_vbus_at_boot == donguru.VbusMode.off()
    s.plug_vbus_at_boot = donguru.VbusMode.on()
    assert s.plug_vbus_at_boot.mode == "on"
    s.button_action = "toggle_vbus"
    assert s.button_action == "toggle_vbus"
    with pytest.raises(ValueError):
        s.button_action = "explode"
    s.meter_window = 0.05
    assert s.meter_window == pytest.approx(0.05)
    s.meter_deadband = donguru.Deadband(0.1, 0.02)
    assert s.meter_deadband == donguru.Deadband(0.1, 0.02)
    cfg = donguru.UartConfig(baud=9600)
    s.uart_plug = cfg
    assert s.uart_plug.baud == 9600
    s.reset()
    assert s.plug_vbus_at_boot == donguru.VbusMode.off()


def test_identify(dg, mock):
    dg.leds.identify(1.0)
    assert mock.identified == 1


def test_closed(mock):
    dg = mock.open()
    dg.close()
    dg.close()
    with pytest.raises(donguru.LinkError):
        dg.meter.read()
    assert issubclass(donguru.LinkError, donguru.Error)


def test_no_device():
    with pytest.raises(donguru.NoDevice):
        donguru.open("label:no-such-dongle")


def test_bad_selector():
    with pytest.raises(donguru.Error):
        donguru.open("path:")
