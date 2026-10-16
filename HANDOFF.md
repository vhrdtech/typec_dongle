# fw25 handoff (FW-25 b129a-no-swd)

Done
- Static review v0.6.0 (13 Oct, worked) .. main: SWD wire path unchanged (formatting only; Orientation::Auto = Normal;
  CMSIS-DAP on fixed EP3). Embassy fork (17b1c24) differs from crates.io 0.6.0 only in src/usb + wait.rs.
  power.rs (PA6, PC14, PD1, PA9, PA5) does not touch SBU / CC pins. Recorded in FEATURES FW-25.
- Prebuilt release images with `plug-vbus-on`, each with wire_weaver at the matching date, in
  /home/roman/.claude/jobs/eee60149/tmp/bisect/{v006,v009,v011}/typec_dongle/fw/donguru/target/thumbv6m-none-eabi/release/donguru
  (git worktrees, detached; remove with `git worktree remove` in both repos when done).

Blocked
- 16 Oct morning: no J-Link and no c0de:cafe on omarchy-m1 USB (Pixel on port 5-1 since 00:45). Told parent.

Next (once the board is back, say in tpm before/after flashing: shared with power-meter, defmt-log)
1. Flash v0.11.0 (`probe-rs run --chip STM32G0B1CEUx <elf>` or `cargo run --release --features plug-vbus-on` in
   the bisect worktree), `probe-rs read --probe c0de:cafe --chip STM32G0B1CEUx b32 0x08000000 16` in both
   `plug.orientation`s. Works -> bisect c47fcf5..main. Fails -> v0.6.0; fails too -> hardware (cable, breakout,
   B129A SWD with J-Link directly, TP1 SWD_SEL level).
2. Scope / J-Link: PD0/PD2 levels during DAP_Connect, GPIOA MODER (0x50000000) for PA0/PA1 while connected.
3. When it reads: close API-8's open probe-rs check (64 KB, both orientations) in FEATURES, CHANGELOG entry.
