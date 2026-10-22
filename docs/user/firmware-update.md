# Firmware update

Donguru's firmware is updated over its own USB connection, no debug probe needed. The firmware reboots into the
STM32's built-in ROM bootloader (USB DFU), the new image is written there, and the bootloader starts the firmware
again. The whole update takes about 45 seconds.

You need the WireWeaver CLI `ww` with `ww dfu stm32` (wire_weaver master since 21 Oct 2026) and the dongle's USB
serial number:

```sh
ww list
```

```
LOCATION                   PRODUCT  SERIAL        API                LABEL
usb 001-1.1.3.1 c0de:cafe  donguru  204B319B5431  donguru_api@0.5.1  donguru-m1
```

The serial number is the same in the firmware and in the ROM bootloader, so `ww` follows the dongle through the whole
update. 
## With `dg fw update`

```sh
dg fw version                 # what runs now
dg fw update donguru.elf      # reboot into the bootloader, write, verify, start, wait
```

```
204B319B5431: donguru 0.1.1 (7d794b3-dirty, release) -> donguru 0.1.1 (10af2b7, release)
415288 bytes in 44.5 s
```

Without a file, `dg fw update` takes `fw/donguru/target/thumbv6m-none-eabi/release/donguru` of the current directory, where
`cargo build --release` in this repository puts it; `-` reads the ELF from stdin. A progress bar on stderr shows the
steps (hidden with `-q` or when stderr is not a terminal). `--format json` prints one object with `serial`, `old`,
`new`, `bytes` and `seconds` on stdout. Exit codes are the common ones: 3 no dongle, 4 the dongle refused (a firmware
without `device.boot`), 5 timeout (the bootloader or the firmware did not show up), 1 the write failed.

With several dongles connected, pick one with `-d SERIAL`.

## Steps with `ww`

The same in two steps with the WireWeaver CLI:

```sh
ww dfu -s 204B319B5431 reboot                          # into the ROM bootloader
ww dfu stm32 -s 204B319B5431 flash donguru.elf --wait 10 --follow
```

`--wait` waits for the bootloader to appear, `--follow` for the firmware to come back after writing. In between the
dongle shows up as `0483:df11 STM Device in DFU Mode` (`ww dfu stm32 list`). The USB hub in the dongle stays up the
whole time, so other devices behind it are not disturbed until the new firmware starts.

`ww dfu -s SERIAL reboot --into app` restarts the firmware without updating.

From a checkout of this repository, `just update SERIAL` builds the release firmware and does both steps.

## If something goes wrong

- The dongle stays in the ROM bootloader (`ww dfu stm32 list` shows it): run the `flash` command again.
- A power cycle (unplug and plug in) always starts the firmware that is in flash.
- A firmware that does not start at all cannot reach the bootloader on rev A boards: flash it with a debug probe
  (`cd fw/donguru && cargo run --release`). A way back over USB alone is planned (FW-45).
