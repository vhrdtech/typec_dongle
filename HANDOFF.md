# Handoff: plug-power (API-11, FW-8, FW-14)

## Done
- `donguru_power` crate (no_std, host tests): `Switch` = mode, state, guard (V / |I| / |P|), FLTB debounce (2
  samples), `vbus_cycle` start/end with a generation token, `Auto` refused on a switch without current measurement.
- fw `src/power.rs`: pins PA6 (EN), PC14 (FLTB, pull-up), PD1 (5V_EN), PA9 (DATA_CUT), PA5 (relay); boot all off;
  `on_adc()` from the ADC task (~15 ms) with peak V / |I| of the whole half buffer (`adc.rs` peak_* fields);
  `power_task` runs the cycle off time; server_task wakes on `power::wait_event()` to mark observed props and send
  the deferred `plug_vbus_cycle` answer.
- `plug-vbus-on` feature removed (boot default via `device.settings.plug_vbus_at_boot` is API-19).
- Docs: `docs/user/power.md`.

## Blocked / next
- fw doesn't build until wire_weaver generates `<prefix>_<method>_send_return` for nested levels: child session
  ww-deferred-nested (wire_weaver SRV-20). Then: build, flash (say to device-contract / other fw sessions first),
  bench test (task list in the tpm task), fw-size delta, FEATURES/CHANGELOG, API-8 probe-rs open check.
