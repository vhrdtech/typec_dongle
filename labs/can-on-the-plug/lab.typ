#import "../lib.typ": *

#show: lab.with(
  title: [CAN through the USB-C plug],
  time: "45 min",
  needs: ([Donguru], [a board with a CAN controller (our B129A)], [a CAN transceiver if the board's bus is
    differential]),
  features: ("API-31", "CLI-41", "FW-36", "CORE-4"),
)

== Goal

Talk CAN to a board over its USB-C connector: the STM32 in Donguru has two FDCAN controllers, and their pins reach the
plug's SBU and A2/B2 lines.

== Setup

Logic-level CAN (TX / RX, no transceiver) between two microcontrollers works directly. For a real differential CAN
bus, put a transceiver at the board's side.

== Steps

#steps(cli: [
  ```sh
  dg can open --pins a2b2 --bitrate 500k          # FDCAN2 on A2 (TX) / B2 (RX)
  dg can dump                                      # 123#DEADBEEF ... (candump format)
  dg can send 321#0102030405060708
  dg can open --pins sbu --bitrate 1M --fd 4M      # FDCAN1 on SBU, CAN FD
  ```
], python: [
  ```python
  import donguru, can                  # python-can
  dg = donguru.open()
  bus = can.Bus(interface="donguru", channel="a2b2", bitrate=500_000)
  bus.send(can.Message(arbitration_id=0x321, data=[1, 2, 3]))
  print(bus.recv(timeout=1.0))
  ```
])

== What you should see

Frames from the board appear in `dg can dump` with timestamps; `send` reaches it. On Linux, `dg can open --socketcan`
could expose it as `can0` for the usual tools.

#safety[A2 and B2 have ESD protection but no over-voltage protection: 3.3 V logic only, never a CAN bus' differential
  lines directly.]

== What next

- Lab "A rack console": CAN boards in a rack, each on a Donguru.
