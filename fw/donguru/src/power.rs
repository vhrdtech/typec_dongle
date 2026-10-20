//! Plug VBUS (U601 AOZ1375, PA6 / FLTB PC14), header VBUS_SW (Q102, PD1), plug data switch (U701, PA9) and the
//! relay (U101, PA5): API-11, FW-8, FW-14.
//!
//! The modes, states and the guard are `donguru_power::Switch`, unit-tested on the host; this module owns the pins,
//! feeds the guard from every ADC update ([`on_adc`], called by the ADC task, ~15 ms) and runs `plug.vbus_cycle`'s
//! off time in [`power_task`]. Changes the server didn't make itself (a trip, a fault, the end of a cycle) mark the
//! observed property changed through the observe signal (`crate::ww::OBSERVE`, WireWeaver SRV-25), which also wakes
//! the server task; the deferred answer of a cycle comes through [`wait_event`] and [`take_cycle_done`].
//!
//! Boot state is everything off: both switches `ForceOff`, data connected, relay open (hw_b153a.md §14), the same
//! as the pull-downs give a halted MCU.

use core::cell::RefCell;

use cnt::cnt;
use defmt::{info, warn};
use donguru_api::{DataPath, GuardTrip, VbusMode, VbusState};
use donguru_power::{CycleError, ModeError, Sample, Switch};
use embassy_futures::select::{Either, select};
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use ww_si::Second;

use crate::{PwrResources, adc, swd};

/// Which VBUS switch.
#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Which {
    Plug,
    Header,
}

struct Power {
    plug: Switch,
    header: Switch,
    /// PWR_EN, U601 EN (R601 pull-down).
    plug_en: Output<'static>,
    /// ~PWR_FLT, U601 FLTB: open drain, no external pull-up, so the MCU's.
    plug_flt: Input<'static>,
    /// 5V_EN, Q101/Q102 (R109 pull-down).
    header_en: Output<'static>,
    /// DATA_CUT, U701 S (R702 pull-down): high = DUT data disconnected.
    data_cut: Output<'static>,
    /// RELAY_EN, U101 LED through R106: high = RL1-RL2 closed.
    relay: Output<'static>,
}

impl Power {
    fn switch(&mut self, which: Which) -> &mut Switch {
        match which {
            Which::Plug => &mut self.plug,
            Which::Header => &mut self.header,
        }
    }

    /// Drives the enable pins from the switch states; called after every change.
    fn apply(&mut self) {
        self.plug_en.set_level(Level::from(self.plug.enabled()));
        self.header_en.set_level(Level::from(self.header.enabled()));
    }
}

static POWER: Mutex<CriticalSectionRawMutex, RefCell<Option<Power>>> =
    Mutex::new(RefCell::new(None));
/// A cycle ended: its deferred answer is in [`CYCLE_DONE`].
static EVENT: Signal<CriticalSectionRawMutex, ()> = Signal::new();
/// A started cycle for [`power_task`].
static CYCLE: Signal<CriticalSectionRawMutex, CycleRequest> = Signal::new();
/// Finished cycles for the server to answer.
static CYCLE_DONE: Channel<CriticalSectionRawMutex, CycleDone, 2> = Channel::new();

#[derive(Clone, Copy)]
struct CycleRequest {
    off_ms: u32,
    token: u32,
    seq: u32,
    started: Instant,
}

/// The deferred answer of a `plug.vbus_cycle`.
pub struct CycleDone {
    /// Request seq from `cx.reply_to()`, 0 when the host expects no answer.
    pub seq: u32,
    pub result: Result<VbusState, ModeError>,
}

/// Takes the pins and sets the boot state: all off, data connected, relay open.
pub fn init(r: PwrResources) {
    let power = Power {
        plug: Switch::new(true),
        // VBUS_SW comes off the receptacle before the shunt: its current isn't measured
        header: Switch::new(false),
        plug_en: Output::new(r.pa6, Level::Low, Speed::Low),
        plug_flt: Input::new(r.pc14, Pull::Up),
        header_en: Output::new(r.pd1, Level::Low, Speed::Low),
        data_cut: Output::new(r.pa9, Level::Low, Speed::Low),
        relay: Output::new(r.pa5, Level::Low, Speed::Low),
    };
    POWER.lock(|p| p.replace(Some(power)));
}

fn with<R>(f: impl FnOnce(&mut Power) -> R) -> R {
    POWER.lock(|p| {
        let mut p = p.borrow_mut();
        // init() runs before the tasks that call in here are spawned
        let p = defmt::unwrap!(p.as_mut());
        f(p)
    })
}

/// The guard's view of the newest ADC update: the worst voltage and current of it.
fn sample() -> Option<Sample> {
    adc::latest().map(|r| Sample {
        vbus_mv: r.peak_input_voltage_mv as i32,
        current_ma: r.peak_input_current_ma as i32,
    })
}

/// Sets the mode of a switch, through the guard. The pin follows at once.
pub fn set_mode(which: Which, mode: VbusMode) -> Result<(), ModeError> {
    let s = sample();
    let res = with(|p| {
        let res = p.switch(which).set_mode(mode, s);
        p.apply();
        res
    });
    match res {
        Ok(()) => info!("{} VBUS mode set, {}", which, state_name(state(which))),
        Err(e) => warn!("{} VBUS mode refused: {}", which, error_name(e)),
    }
    res
}

pub fn mode(which: Which) -> VbusMode {
    with(|p| p.switch(which).mode().clone())
}

pub fn state(which: Which) -> VbusState {
    with(|p| p.switch(which).state())
}

/// One ADC update (from the ADC task): runs the guard and watches U601 FLTB.
pub fn on_adc() {
    let s = sample();
    let changed = with(|p| {
        let fault = p.plug_flt.is_low();
        let plug = p.plug.on_sample(s, fault);
        let header = p.header.on_sample(s, false);
        if plug || header {
            p.apply();
        }
        (
            plug.then_some(p.plug.state()),
            header.then_some(p.header.state()),
        )
    });
    if let (Some(st), _) | (_, Some(st)) = changed {
        match st {
            VbusState::Fault => {
                cnt!(plug_vbus_faults: u32, warn);
            }
            _ => {
                cnt!(vbus_trips: u32, warn);
            }
        }
    }
    if let Some(st) = changed.0 {
        log_cut("plug VBUS cut", st, s);
        crate::ww::OBSERVE.mark_changed_plug_vbus();
    }
    if let Some(st) = changed.1 {
        log_cut("header VBUS_SW cut", st, s);
        crate::ww::OBSERVE.mark_changed_header_vbus_sw();
    }
}

/// Starts `plug.vbus_cycle`: optionally SBU to Hi-Z first, then plug VBUS off; [`power_task`] switches it back on
/// after `off` and hands the answer for `seq` to the server ([`take_cycle_done`]).
pub async fn start_cycle(off: &Second, release_sbu: bool, seq: u32) -> Result<(), CycleError> {
    if release_sbu {
        // before VBUS goes: nothing on SBU may back-power the DUT while it is off
        swd::release().await;
    }
    let res = with(|p| {
        let res = p.plug.cycle_start(off);
        p.apply();
        res
    });
    let (off_ms, token) = res?;
    info!("plug VBUS cycle: off for {} ms", off_ms);
    CYCLE.signal(CycleRequest {
        off_ms,
        token,
        seq,
        started: Instant::now(),
    });
    Ok(())
}

/// Runs the off time of `plug.vbus_cycle`. A new cycle (after a mode write ended the old one) replaces a running
/// one, which is then answered with the state as it is.
#[embassy_executor::task]
pub async fn power_task() {
    let mut req = CYCLE.wait().await;
    loop {
        let until = req.started + Duration::from_millis(req.off_ms as u64);
        match select(Timer::at(until), CYCLE.wait()).await {
            Either::First(()) => {
                finish(req);
                req = CYCLE.wait().await;
            }
            Either::Second(new) => {
                finish(req);
                req = new;
            }
        }
    }
}

fn finish(req: CycleRequest) {
    let s = sample();
    let result = with(|p| {
        let res = p.plug.cycle_end(req.token, s);
        p.apply();
        res
    });
    // the current just before switching back on shows VBUS really was off (~0 mA)
    info!(
        "plug VBUS cycle done after {} ms ({} mA while off): {}",
        req.started.elapsed().as_millis() as u32,
        s.map_or(0, |s| s.current_ma),
        match result {
            Ok(st) => state_name(st),
            Err(e) => error_name(e),
        }
    );
    if CYCLE_DONE
        .try_send(CycleDone {
            seq: req.seq,
            result,
        })
        .is_err()
    {
        cnt!(vbus_cycle_answers_dropped: u32, warn);
    }
    crate::ww::OBSERVE.mark_changed_plug_vbus();
    EVENT.signal(());
}

/// Waits until something changed outside a handler.
pub async fn wait_event() {
    EVENT.wait().await
}

/// A finished cycle to answer, if any.
pub fn take_cycle_done() -> Option<CycleDone> {
    CYCLE_DONE.try_receive().ok()
}

pub fn set_data(path: DataPath) {
    info!(
        "plug data {}",
        if path == DataPath::Cut {
            "cut"
        } else {
            "connected"
        }
    );
    with(|p| p.data_cut.set_level(Level::from(path == DataPath::Cut)));
}

pub fn data() -> DataPath {
    if with(|p| p.data_cut.is_set_high()) {
        DataPath::Cut
    } else {
        DataPath::Connected
    }
}

pub fn set_relay(closed: bool) {
    info!("relay {}", if closed { "closed" } else { "open" });
    with(|p| p.relay.set_level(Level::from(closed)));
}

pub fn relay() -> bool {
    with(|p| p.relay.is_set_high())
}

// Names for the log: `Debug2Format` would pull in `core::fmt` and every `Debug` impl (~20 KiB here).
fn state_name(st: VbusState) -> &'static str {
    match st {
        VbusState::Off => "Off",
        VbusState::On => "On",
        VbusState::Fault => "Fault",
        VbusState::Tripped(GuardTrip::OverVoltage) => "Tripped(OverVoltage)",
        VbusState::Tripped(GuardTrip::OverCurrent) => "Tripped(OverCurrent)",
        VbusState::Tripped(GuardTrip::OverPower) => "Tripped(OverPower)",
        #[allow(unreachable_patterns)] // future variants
        _ => "?",
    }
}

fn error_name(e: ModeError) -> &'static str {
    match e {
        ModeError::Invalid => "invalid limits",
        ModeError::OverVoltage { .. } => "VBUS over max_voltage",
        ModeError::NoMeasurement => "no measurement yet",
        ModeError::CurrentNotMeasured => "Auto without a current measurement",
    }
}

fn log_cut(what: &str, st: VbusState, s: Option<Sample>) {
    let s = s.unwrap_or(Sample {
        vbus_mv: 0,
        current_ma: 0,
    });
    warn!(
        "{}: {} at {} mV, {} mA",
        what,
        state_name(st),
        s.vbus_mv,
        s.current_ma
    );
}
