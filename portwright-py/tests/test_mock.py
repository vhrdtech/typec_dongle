"""The `portwright` package against portwright-core's in-process mock dongle (a B153A Main at boot)."""

import gc

import pytest

import portwright
from portwright.mock import MockDevice


@pytest.fixture
def mock():
    return MockDevice()


@pytest.fixture
def pw(mock):
    with mock.open() as pw:
        yield pw


def test_version():
    assert portwright.__version__.count(".") == 2


def test_info(pw):
    info = pw.info()
    assert info.variant == "Main"
    assert info.uid == "0049002A3133510B37363436"
    assert pw.entry.serial == pw.serial
    assert "Dongle(" in repr(pw)


def test_label(pw):
    assert pw.label == ""
    pw.label = "bench-1"
    assert pw.label == "bench-1"
    assert pw.info().label == "bench-1"


def test_power_on_off(pw, mock):
    assert pw.power.state.state == "off"
    status = pw.power.on()
    assert status.on and not status.tripped
    assert str(status) == "on"
    assert pw.power.mode.mode == "on"
    assert pw.power.mode.max_voltage == pytest.approx(5.5)
    assert mock.plug_vbus_mode.mode == "on"
    assert not pw.power.off().on
    assert mock.plug_vbus_mode == portwright.VbusMode.off()


def test_power_trip(pw, mock):
    pw.power.on(trip=1.5)
    mode = pw.power.mode
    assert mode.mode == "auto"
    assert mode.max_current == pytest.approx(1.5)
    assert mode.max_power == pytest.approx(1.5 * 5.5)
    assert repr(mode) == "VbusMode.auto(max_current=1.5, max_voltage=5.5)"


def test_power_cycle(pw, mock):
    pw.power.on()
    status = pw.power.cycle(off=0.05)
    assert status.on
    assert mock.vbus_cycles == 1


def test_power_mode_setter(pw):
    pw.power.mode = portwright.VbusMode.auto(max_current=0.5, max_voltage=5.25)
    assert pw.power.mode == portwright.VbusMode.auto(0.5, 5.25)


def test_bad_values(pw):
    with pytest.raises(ValueError):
        pw.power.on(max_voltage=float("nan"))
    with pytest.raises(ValueError):
        portwright.VbusMode.on(max_voltage=1e12)


def test_meter_read(pw, mock):
    r = pw.meter.read()
    assert r.voltage == pytest.approx(5.0)
    assert r.current == 0.0
    mock.set_reading(voltage=5.02, current=0.25)
    r = pw.meter.read()
    assert r.voltage == pytest.approx(5.02)
    assert r.current == pytest.approx(0.25)
    assert r.power == pytest.approx(5.02 * 0.25, rel=1e-3)


def test_meter_watch(pw, mock):
    mock.set_reading(voltage=5.0, current=0.1)
    readings = list(pw.meter.watch(interval=0.02, count=3))
    assert len(readings) == 3
    assert all(r.current == pytest.approx(0.1) for r in readings)


def test_meter_watch_close(pw):
    with pw.meter.watch(interval=0.02) as w:
        first = next(w)
        assert first.voltage == pytest.approx(5.0)
    with pytest.raises(StopIteration):
        next(w)


def test_meter_watch_flags(pw):
    with pytest.raises(ValueError):
        pw.meter.watch(on_change=True, samples=True)


def test_meter_totals(pw, mock):
    mock.set_totals(charge=3.6, energy=18.0, peak_current=1.2, since=60.0)
    t = pw.meter.totals()
    assert t.charge == pytest.approx(3.6)
    assert t.charge_mah == pytest.approx(1.0)
    assert t.energy_wh == pytest.approx(0.005)
    assert t.peak_current == pytest.approx(1.2)
    assert t.since == pytest.approx(60.0)
    pw.meter.reset_totals()
    assert pw.meter.totals().charge == 0.0


def test_meter_settings(pw):
    assert pw.meter.temperature() == pytest.approx(30.0)
    pw.meter.window = 0.1
    assert pw.meter.window == pytest.approx(0.1)
    pw.meter.deadband = portwright.Deadband(voltage=0.05, current=0.01)
    assert pw.meter.deadband == portwright.Deadband(0.05, 0.01)


def test_usb(pw, mock):
    assert pw.usb.connected
    pw.usb.disconnect()
    assert not pw.usb.connected
    assert not mock.data_connected
    pw.usb.connect()
    assert mock.data_connected
    pw.usb.connected = False
    assert not mock.data_connected


def test_header(pw, mock):
    assert not pw.header.relay
    pw.header.relay = True
    assert mock.relay
    assert pw.header.vbus.state == "off"
    assert pw.header.vbus_on().on
    assert pw.header.vbus_mode.mode == "on"
    assert not pw.header.vbus_off().on


def test_uart_configure(pw):
    c = pw.uart.header.configure(baud=1_000_000)
    assert c.baud == 1_000_000
    assert c.parity == "none"
    with pytest.raises(ValueError):
        pw.uart.header.configure(parity="sometimes")


def test_settings(pw):
    s = pw.settings
    assert s.plug_vbus_at_boot == portwright.VbusMode.off()
    s.plug_vbus_at_boot = portwright.VbusMode.on()
    assert s.plug_vbus_at_boot.mode == "on"
    s.button_action = "toggle_vbus"
    assert s.button_action == "toggle_vbus"
    with pytest.raises(ValueError):
        s.button_action = "explode"
    s.meter_window = 0.05
    assert s.meter_window == pytest.approx(0.05)
    s.meter_deadband = portwright.Deadband(0.1, 0.02)
    assert s.meter_deadband == portwright.Deadband(0.1, 0.02)
    cfg = portwright.UartConfig(baud=9600)
    s.uart_plug = cfg
    assert s.uart_plug.baud == 9600
    s.reset()
    assert s.plug_vbus_at_boot == portwright.VbusMode.off()


def test_identify(pw, mock):
    pw.leds.identify(1.0)
    assert mock.identified == 1


def test_closed(mock):
    pw = mock.open()
    pw.close()
    pw.close()
    with pytest.raises(portwright.LinkError):
        pw.meter.read()
    assert issubclass(portwright.LinkError, portwright.Error)


def test_no_device():
    with pytest.raises(portwright.NoDevice):
        portwright.open("label:no-such-dongle")


def test_bad_selector():
    with pytest.raises(portwright.Error):
        portwright.open("path:")


def test_device_error(pw, mock):
    mock.set_settings_fault(1)
    with pytest.raises(portwright.DeviceError) as e:
        pw.settings.button_action = "toggle_data"
    assert e.value.kind == "fault"
    assert e.value.item
    mock.set_settings_fault(None)
    pw.settings.button_action = "toggle_data"


def test_unimplemented(pw):
    # the mock models the header UART only, as a firmware without the plug UART would answer
    with pytest.raises(portwright.Unimplemented):
        pw.uart.plug.configure()


def test_dongle_keeps_its_mock():
    pw = MockDevice().open()
    gc.collect()
    assert pw.meter.read().voltage == pytest.approx(5.0)
