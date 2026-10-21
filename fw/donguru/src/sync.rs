//! Donguru's shared clock (DEV-13, FW-44): host steer points discipline the local clock onto UTC, so the status
//! LEDs (`src/led.rs`) breathe and blink in phase with other boards on the same host. Steered and served as the API
//! (`clock`, `ww_timebase::Clock`) by `src/ww/clock.rs`; this module only holds the driver-level static, as
//! `src/led.rs` holds `LEDS` for the indication.
//!
//! No sync line (nothing to lock to on this board) and no oscillator backend: Donguru's HSI16 trim is too coarse to
//! steer without wobbling UART / PWM, decided 21 Oct 2026 (P2528 design/clock-sync.md).

/// The board's shared clock, read by the LED task every tick and steered from `clock.steer` (`src/ww/clock.rs`).
pub static SYNC: bedrock_sync::Sync = bedrock_sync::Sync::new(bedrock_sync::Config::DEFAULT);
