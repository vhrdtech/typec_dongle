"""A mock dongle in this process (donguru-core's mock), so scripts and their tests run without hardware.

    from donguru.mock import MockDevice
    mock = MockDevice()
    mock.set_reading(voltage=5.02, current=0.25)
    dg = mock.open()
    assert dg.meter.read().current == 0.25
"""

from ._donguru import MockDevice

__all__ = ["MockDevice"]
