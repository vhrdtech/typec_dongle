#import "../lib.typ": *

#show: lab.with(
  title: [Hardware tests with pytest],
  time: "30 min",
  needs: ([Donguru], [a device under test], [Python with uv]),
  features: ("CORE-4", "CORE-9", "API-14"),
)

== Goal

Write tests for a USB device as ordinary pytest tests: a fixture gives each test a freshly powered device and its
console, and puts its power numbers into the report.

== Steps

#steps(python: [
  ```sh
  uv add --dev pytest pytest-donguru
  ```
  ```python
  # test_device.py
  def test_boots(dut):                       # dut: power-cycled, console open, meter running
      dut.console.expect(b"boot ok", timeout=5)

  def test_idle_current(dut):
      dut.console.expect(b"idle")
      assert dut.meter.average(seconds=5).current < 0.020

  def test_survives_unplug(dut):
      dut.usb.cycle(off=1.0, wait_for="c0de:cafe")
      dut.console.expect(b"usb reconnected")
  ```
  ```sh
  uv run pytest --donguru=bench-1 --donguru-uart=115200
  #  test_device.py::test_boots PASSED        [0.31 Wh, peak 0.42 A]
  ```
])

== What you should see

Each test starts from a clean power-on; the report shows energy and peak current per test, and a failing test keeps
the console output in its report.

== What next

- Lab "Test farm": the same tests across ten boards in parallel (`pytest -n 10`).
