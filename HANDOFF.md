# Handoff: swd-fast (P2534#13oct-1353, P2534#13oct-1353b)

## Done

- **Step 1, FW-20** (commits 0ad5462, 64b2c06, babf602): bit-bang SWD on two inline-asm kernels (`out_bits`,
  `in_bits` in `fw/donguru/src/swd.rs`), spin counts from a SysTick calibration at boot (`donguru_dap::swclk`).
  Bench-tested by the parent at 0ad5462: reads identical, 7.03 s -> 1.07 s for 64 KB, memtest passes. The scope
  showed 70 % duty at the top setting; babf602 balances the halves (bit work in the low half, counter in the high
  half, odd spin low) and caps at 3 MHz. **The scope re-check of babf602 is still open** (FW-20 is 🧪 until then).
- **Step 2, FW-21 + API-5** (5c0861b): SPI2 through the TXS0102 (`SwdMode::Spi`, `swd_mode` property,
  `donguru_api` 0.1.2, firmware feature `swd-spi` for the boot default). Built and unit-tested only.

## Next

1. The parent runs the bench checks (this session can't flash: the auto-mode classifier blocks probe-rs on
   omarchy-m1). ELFs in omarchy-m1:/tmp/dap512/: `donguru-fw20-babf602.elf` (duty fix),
   `donguru-fw21-spi-5c0861b.elf` (SPI default), `donguru-fw21-bb-5c0861b.elf` (same code, bit-bang default).
2. Record the results in FEATURES (FW-20, FW-21) and CHANGELOG; flip 🧪 -> ✅ where they pass; set `SPI_MAX_HZ`
   to what is clean.
3. `git rm HANDOFF.md`, `tpm work ready`.

## Decisions and gotchas

- Kernels are `#[inline(never)]` asm so calibration and use run the same code; high registers can only be asm
  clobbers in Thumb-1, so spin counts are loaded into r8/r10/r11/r12 inside the asm.
- SPI uses CPHA 0 for reads too: MISO sampled at the rising edge = the bit-bang's "just before the edge" point.
  MOSI turns back on only where the target is done or drives 0 (OK ack's last bit); relies on SPI2 holding MOSI at
  the last bit sent (0 from the receive frames) between frames: worth a look on the scope.
- `DS` is changed with SPE=1 between frames (never SPE toggling during a session, so SCK never floats into the
  shifter's pull-up).
- Firmware files are formatted at 120 columns (`rustfmt --edition 2024 --config max_width=120 <file>`); don't run
  rustfmt on `main.rs`, it recurses into modules that aren't 120-formatted.
