#import "../lib.typ": *

#show: lab.with(
  title: [Donguru as a dev board],
  time: "an evening",
  needs: ([Donguru], [Rust toolchain], [a second Donguru or any SWD probe, optional]),
  features: ("FW-12", "CLI-10", "FW-1"),
)

== Goal

Treat Donguru as an STM32G0B1 development board with a USB hub, a power switch, a meter, LEDs and a button, and
run your own firmware on it, then put the official one back.

== Steps

#steps(cli: [
  ```sh
  git clone https://github.com/vhrdtech/typec_dongle && cd typec_dongle/fw/donguru
  # change something in src/, then:
  cargo build --release
  dg fw update target/thumbv6m-none-eabi/release/donguru        # through the ROM bootloader, no probe
  # completely your own firmware (embassy, RTIC, C):
  dfu-util -a 0 -s 0x08000000:leave -D my-firmware.bin            # hold the button while plugging in first
  # back to the official firmware:
  dg fw update --official
  ```
])

== What you should see

Your firmware runs; with `probe-rs` through a second Donguru (next lab) you also get a debugger. The ROM bootloader
can't be overwritten, so the official firmware always comes back.

#tip[The board's pins, nets and what each is wired to are in the docs (`hw_b153a.md`): keep the VBUS enables off
  unless your firmware checks the voltage first.]

== What next

- Lab "Donguru debugs Donguru".
