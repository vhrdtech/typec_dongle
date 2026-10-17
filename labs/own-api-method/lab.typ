#import "../lib.typ": *

#show: lab.with(
  title: [Add your own method to the firmware],
  time: "1-2 h",
  needs: ([Donguru], [Rust toolchain], [the typec_dongle repo]),
  features: ("API-29", "CORE-11", "CLI-39", "FW-12", "CLI-10"),
)

== Goal

Add a function of your own to Donguru's firmware (here: blink a word in Morse code on the status LED), call it from
the command line and Python, and keep the official `dg` working next to it.

== Steps

#steps(cli: [
  ```rust
  // fw/donguru/src/ext.rs: your level, reserved for user code in DonguruApi as `ext`
  impl Ext for Server {
      async fn morse(&mut self, text: &str) -> Result<(), ExtError> {
          leds::morse(text).await
      }
  }
  ```
  ```sh
  cd fw/donguru && cargo build --release --features ext
  dg fw update target/thumbv6m-none-eabi/release/donguru
  dg call ext.morse "SOS"        # any method, found through the firmware's introspection
  dg info                        # shows "custom firmware: ext.morse"
  ```
], python: [
  ```python
  import donguru
  dg = donguru.open()
  dg.call("ext.morse", text="SOS")
  ```
])

== What you should see

The LED blinks · · · – – – · · ·. `dg power`, `dg uart` and the rest keep working: your method lives on its own
level, so the official API's version stays compatible.

== What next

- Lab "Donguru as a dev board": replace the firmware completely.
