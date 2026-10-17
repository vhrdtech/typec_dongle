#import "../lib.typ": *

#show: lab.with(
  title: [Reviving a hung board],
  time: "20 min",
  needs: ([Donguru], [a board that sometimes hangs: a Raspberry Pi, a dev board on a long test]),
  features: ("CLI-11", "CLI-33", "FW-31", "API-15"),
)

== Goal

Power-cycle a device automatically when it stops answering: first from a script on the computer, then from
Donguru alone, watching a heartbeat pin, so it works even when the computer is busy or asleep.

== Steps

#steps(cli: [
  ```sh
  # host side: ping the board, cycle it after 3 missed answers in a row
  dg watchdog --check "ping -c1 -W2 raspberrypi.local" --every 10s --misses 3 --off 2s
  #  12:01:10 ok · 12:01:20 ok · 12:01:30 miss 1 · ... · 12:01:50 cycled (3 misses) · 12:02:31 ok
  # on the dongle: the board toggles a pin at least every 5 s, or Donguru cycles it
  dg watchdog device --pin io7 --timeout 5s --off 2s --grace 60s   # grace: time to boot
  dg watchdog device --status       # armed, last beat 1.2 s ago, 3 cycles since Monday
  ```
], python: [
  ```python
  import donguru, subprocess, time
  dg = donguru.open()
  misses = 0
  while True:
      ok = subprocess.run(["ping", "-c1", "-W2", "raspberrypi.local"]).returncode == 0
      misses = 0 if ok else misses + 1
      if misses == 3:
          dg.power.cycle(off=2.0); misses = 0; time.sleep(60)
      time.sleep(10)
  ```
])

== What you should see

`dg watchdog` prints each check and every cycle with its reason; `-f jsonl` makes it a log for later. The device
watchdog keeps counting cycles in the dongle, so you see how often the board hung over a week.

#tip[The heartbeat can also be a line on the board's UART (`--uart "alive"`), so a board without a spare pin works
  too.]

== What next

- Lab "Test farm": a watchdog for each board in a rack.
