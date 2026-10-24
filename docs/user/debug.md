---
icon: lucide/bug
---

# Bring-up and debug

`dg debug` reaches below the functions: the MCU's pins one by one, the pins as they were at boot, the hub's
registers, the two UCPD ports and the peripheral registers. It is for bringing up a board and for production
tests, not for normal use: a pin changed here can disagree with what `dg power` or `dg header` report until they
write it again.

The firmware decides what may change; a refusal comes back with its reason and exit code 4.

## Pins

Pins go by their net name from the schematic (B153A, `hw_b153a.md` §11) or by port pin. Tab completion lists the
nets with their pin and what they do.

```sh
dg debug pin get RELAY_EN          # PA5 RELAY_EN: analog, reads low
dg debug pin set RELAY_EN high     # push-pull output, high: the relay clicks
dg debug pin set PB12 input-pull-up
dg debug pin set SBU1 open-drain low
dg debug pin af IO9 1              # alternate function 0..=7
```

`set` takes `high` / `low` (a push-pull output at that level) or a mode: `analog`, `input`, `input-pull-up`,
`input-pull-down`, `push-pull`, `open-drain`, `open-drain-pull-up`, with an optional level for the output modes.

What is refused:

- **Always**: NRST, the pins tied to a supply, our own USB and SWD, `CC_DA_EN` (Rp on the computer's CC on rev A),
  the VBUS enables (use `dg power` and `dg header`, they keep the guard), the hub's reset and I2C (use
  `dg debug hub` and `dg hub recover`).
- **While a function holds the pin**: the LEDs, the meter and button inputs, the header UART (IO7 / IO8 and PB1,
  which is tied to IO7), V_SBU select, an SWD session (the SBU pins, the level shifter's OE, the pull-ups, the plug
  CC drive) and a UCPD role (its CC pins). The error names the function.
- **When the pins would fight each other on the board**: two pins of one net both driving (PB1 / PB3, PB6 / PB9 /
  PB11, PB7 / PB8), `SBU_OE` high while PA0 / PA1 drive the SBU lines, PA0 / PA1 push-pull without V_SBU at 3.3 V,
  `~PWR_FLT` push-pull. A state that already breaks a rule doesn't block the change that fixes it.

An SWD session starting (`DAP_Connect`) takes its pins back as it needs them.

## Pin dump and the boot state

```sh
dg debug pins                  # every bonded pin: net, mode, the level it reads
dg debug pins --diff           # only the pins not in their reset state
dg debug pins --boot --diff    # the pins as captured at boot, against the reset state: empty on a healthy board
```

The firmware reads GPIOA..F first thing after reset, before it sets anything up. `--boot --diff` printing
`every pin in its reset state` means nothing (a stuck pin, a wrong strap) changed them before the firmware ran.
The level a pin reads (IDR) is not compared. `--format json` / `jsonl` give one object per pin.

## Hub registers

The hub (U301) on its SMBus, through the firmware, which owns that bus:

```sh
dg debug hub read 0x00 4       # VID and PID, low byte first
dg debug hub write 0x07 0x00 0x00   # REG BYTE...
```

`read` takes 1..=32 bytes, `write` 1..=32. STATUS/COMMAND (0xFF) is refused: its reset and attach bits drop the
dongle's own USB link; `dg hub recover` does that on purpose. A NAK from the hub is a device error, a busy bus
(the watchdog probing the hub) answers busy, try again.

## UCPD

```sh
dg debug ucpd                  # both ports: role, CC1 / CC2 state, CR and SR
dg debug ucpd 2 --role rd      # plug side as a sink
dg debug ucpd 2 --role off
```

UCPD1 is the receptacle (to the computer), UCPD2 the plug (to the device under test). Roles: `off`, `rd`,
`rp0a9`, `rp1a5`, `rp3a0`. On rev A, UCPD1 may only be `off` or `rd`: an Rp there would present a source to the
computer. A port with a role holds its CC pins; on the plug side, an SWD session's CC drive holds them too and the
role is refused while it runs.

## Peripheral registers

```sh
dg debug reg 0x50000000 10     # GPIOA, 10 words
dg debug reg 0x40021000        # RCC_CR
```

Reads only, word aligned, 1..=32 words, and only inside the blocks of the memory map that hold a peripheral (a
reserved address would fault the MCU). There are no memory reads or writes. Reading a data register can take
what it reads (a UART's RDR, an SPI FIFO).
