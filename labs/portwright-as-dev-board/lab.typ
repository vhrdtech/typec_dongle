#import "../lib.typ": *

#show: lab.with(
  title: [Portwright as a dev board],
  time: "an evening",
  needs: ([Portwright], [Rust toolchain], [a second Portwright or any SWD probe, optional]),
  features: ("FW-12", "CLI-10", "FW-1"),
)

== Goal

Treat Portwright as an STM32G0B1 development board with a USB hub, a power switch, a meter, LEDs and a button, and
run your own firmware on it, then put the official one back.

== Steps

#steps(cli: [
  ```sh
  git clone https://github.com/vhrdtech/typec_dongle && cd typec_dongle/fw/portwright
  # change something in src/, then:
  cargo build --release
  pw fw update target/thumbv6m-none-eabi/release/portwright        # through the ROM bootloader, no probe
  # completely your own firmware (embassy, RTIC, C):
  dfu-util -a 0 -s 0x08000000:leave -D my-firmware.bin            # hold the button while plugging in first
  # back to the official firmware:
  pw fw update --official
  ```
])

== What you should see

Your firmware runs; with `probe-rs` through a second Portwright (next lab) you also get a debugger. The ROM bootloader
can't be overwritten, so the official firmware always comes back.

#tip[The board's pins, nets and what each is wired to are in the docs (`hw_b153a.md`): keep the VBUS enables off
  unless your firmware checks the voltage first.]

== What next

- Lab "Portwright debugs Portwright".
