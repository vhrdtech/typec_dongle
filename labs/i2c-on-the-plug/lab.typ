#import "../lib.typ": *

#show: lab.with(
  title: [I2C on the plug],
  time: "20 min",
  needs: ([Portwright], [an I2C sensor breakout (BME280, SHT40)], [the SBU breakout, 4 wires]),
  features: ("FW-10", "API-16", "CLI-12", "PKG-6", "CORE-4"),
)

== Goal

Talk to an I2C chip over the USB-C plug's SBU lines: scan the bus, read a register, read a temperature.

== Setup

SBU1 → SDA, SBU2 → SCL, VBUS or 3.3 V and GND to the sensor. Portwright's pull-ups to its SBU level are switchable.

#photo[A BME280 breakout wired to the SBU breakout on Portwright's plug]

== Steps

#steps(cli: [
  ```sh
  pw i2c scan --pullups                 # 0x76
  pw i2c read 0x76 0xD0 1               # chip id: 0x60 = BME280
  pw i2c write 0x76 0xF4 0x27           # normal mode
  pw i2c read 0x76 0xF7 8 -f json       # raw pressure and temperature bytes
  ```
], python: [
  ```python
  import portwright
  pw = portwright.open()
  bus = pw.i2c.plug(pullups=True, speed=400_000)
  print(hex(bus.read_reg(0x76, 0xD0)))      # 0x60
  ```
])

== What you should see

`scan` lists the sensor's address. If the cable is in the other way round, SDA and SCL swap: Portwright swaps them back
from the CC orientation, or `pw sbu orientation swapped` does it by hand.

== What next

- Lab "Header GPIO and ADC": the same from the 2.54 mm header, no breakout needed.
