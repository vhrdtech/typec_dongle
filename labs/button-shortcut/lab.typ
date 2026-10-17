#import "../lib.typ": *

#show: lab.with(
  title: [The button as a shortcut],
  time: "5 min",
  needs: ([Donguru], [anything on its plug]),
  features: ("API-13", "API-19", "CLI-22", "CLI-27"),
)

== Goal

Make Donguru's button do something useful: power-cycle the device under test without a computer command, or run a
script on the computer.

== Steps

#steps(cli: [
  ```sh
  dg input watch                          # press the button: Pressed, Released (held 0.12 s)
  dg settings set button_action cycle-vbus    # stored in the dongle, works without a computer
  dg settings set button_action off           # back to nothing
  dg input on press -- ./flash.sh         # runs flash.sh on every press while this command runs
  ```
], python: [
  ```python
  import donguru
  dg = donguru.open()
  for ev in dg.inputs.events():           # Pressed / Released(held=...) / Switched(to=...)
      if ev.kind == "released" and ev.held > 2.0:
          print("long press: flashing"); flash()
  ```
])

== What you should see

With `cycle-vbus`, a short press turns the plug off for the cycle time and back on, and the status LED blinks once.
`dg input on press` prints each press and the exit code of the command it ran.

#tip[The slide switch is an input too: `dg input watch` shows `Switched A -> B`, so a script can use it as a mode
  switch, e.g. "B = flash the release build".]

== What next

- Lab "Hung board": the same cycle triggered by a script instead of a finger.
