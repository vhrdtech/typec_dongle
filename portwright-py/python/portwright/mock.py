"""A mock dongle in this process (portwright-core's mock), so scripts and their tests run without hardware.

    from portwright.mock import MockDevice
    mock = MockDevice()
    mock.set_reading(voltage=5.02, current=0.25)
    pw = mock.open()
    assert pw.meter.read().current == 0.25
"""

from ._portwright import MockDevice

__all__ = ["MockDevice"]
