//! The PD source on the plug CC (`plug.pd`, API-30): usbpd's source policy engine on UCPD2 through embassy's UCPD
//! driver, plug VBUS through `src/power.rs`, every message into the log ring the server sends as `plug.pd.messages`.
//! What needs no hardware (the offer, the answer to a Request, the ring, the phases) is `donguru_board::pd`.
//!
//! The server sets the source ([`set`]): it claims the plug CC pins (`Holder::PdSource`, so an SWD session's CC drive
//! and `debug.ucpd_role` stay off them) and hands the offer to [`pd_task`]; `Off` goes the same way and the task gives
//! the pins back once UCPD2 is off. Per attach the task:
//!
//! 1. presents the offer's Rp on CC1 and waits for it to read Rd (`Low` in source mode, RM0444 table 237; embassy's
//!    UCPD test waits for the same) for tCCDebounce. CC1 only: the plug has one CC contact, A5 (PD0); PD2 is its
//!    VCONN contact B5 (hw_b153a.md §5), so CC2's PHY stays off and no VCONN is given. The flip is the DUT's to see,
//!    in its receptacle;
//! 2. turns plug VBUS on in the guarded mode of the offer (`Offer::vbus_mode`) and runs the policy engine until the
//!    sink detaches (the attached line leaves Rd for tPDDebounce) or the setting changes;
//! 3. turns VBUS off and holds it off for tSrcRecover before the next attach, as rev A cannot read plug VBUS to see
//!    vSafe0V (hw_b153a.md §3, FW-35).
//!
//! A hard reset (either way) turns VBUS off for tSrcRecover and on again (`DevicePolicyManager::hard_reset`). A
//! policy engine that gives up (no answer after nHardResetCount) leaves VBUS off until the next attach.
//!
//! ES0548 2.17.1 (TXHRST at a TX underflow corrupts the next CRC, all revisions): embassy's driver transmits by DMA,
//! the workaround the errata gives. 2.17.2 and 2.17.3 have none and need none here.
//!
//! The task runs on an interrupt executor of its own, above the thread-mode tasks and below every interrupt ([start]):
//! embassy's driver arms the RX DMA only in `receive`, so the GoodCRC of a message we sent is lost when `receive` comes
//! too late after the end of the TX. The FUSB302B's GoodCRC starts about 100 us after our EOP, and its first bytes
//! are in RXDR about 300 us later; on the thread-mode executor a turn of the server task (`pd_pump`) took longer than
//! that about once in 30 messages (FW-62). Its statics are all behind critical sections, as for any interrupt.
//!
//! Not done: PD 3.0 collision avoidance (SinkTxOk / SinkTxNG by Rp, the source keeps one Rp), so a sink that starts
//! an AMS of its own may collide with one of ours; usbpd starts none after the contract.

use core::cell::{Cell, RefCell};

use cnt::cnt;
use defmt::{info, warn};
use donguru_api::{PdSource, VbusMode};
use donguru_board::pd::{
    CC_DEBOUNCE_MS, Kind, Offer, OfferError, Phase, Record, Ring, Rp, SRC_RECOVER_MS, TxOutcome,
};
use donguru_board::pins::Holder;
use embassy_executor::InterruptExecutor;
use embassy_futures::select::{Either, select};
use embassy_stm32::interrupt::{InterruptExt, Priority};
use embassy_stm32::ucpd::{self, CcPhy, CcPull, CcSel, CcVState, PdPhy, Ucpd};
use embassy_stm32::{interrupt, peripherals};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer, with_timeout};
use portable_atomic::{AtomicU32, Ordering};
use usbpd::protocol_layer::message::data::request::PowerSource;
use usbpd::protocol_layer::message::data::source_capabilities::SourceCapabilities;
use usbpd::source::device_policy_manager::{
    CapabilityResponse, DevicePolicyManager, DrpDevicePolicyManager, EprDevicePolicyManager,
    SourceDpm,
};
use usbpd::source::policy_engine::Source;
use usbpd_traits::{Driver, DriverRxError, DriverTxError};

use crate::{Irqs, PdResources, pins, power};

/// Records the ring holds until the server sends them: a contract is about 10, a hard-reset loop more.
const LOG_LEN: usize = 48;
/// tPDDebounce (10-20 ms): the attached line must stay off Rd this long for a detach.
const PD_DEBOUNCE_MS: u64 = 15;
/// VBUS on until the capabilities go out: U601's soft start (1 nF, a few ms) and the DUT's input capacitance,
/// well inside tVBUSON (275 ms). Rev A cannot read plug VBUS (FW-35).
const VBUS_SETTLE_MS: u64 = 50;

static LOG: Mutex<CriticalSectionRawMutex, RefCell<Ring<LOG_LEN>>> =
    Mutex::new(RefCell::new(Ring::new()));
/// A new setting for the task, `None` for off.
static CONTROL: Signal<CriticalSectionRawMutex, Option<Offer>> = Signal::new();
/// The setting and where the source stands, for the server's reads.
static SHARED: Mutex<CriticalSectionRawMutex, Cell<(Option<Offer>, Phase)>> =
    Mutex::new(Cell::new((None, Phase::Off)));
/// SenderResponseTimer of the running offer, ms, for [PdTimer] (whose `after_millis` has no `self`).
static SENDER_RESPONSE_MS: AtomicU32 =
    AtomicU32::new(donguru_board::pd::SPEC_SENDER_RESPONSE_MS as u32);

/// Why [set] refused a setting.
pub enum SetError {
    Invalid(OfferError),
    InUse(Holder),
}

/// Starts, changes or stops the source. Answers at once; the task follows.
pub fn set(source: &PdSource) -> Result<(), SetError> {
    let offer = Offer::from_setting(source).map_err(SetError::Invalid)?;
    if offer.is_some() && !pins::holds(Holder::PdSource) {
        pins::claim(Holder::PdSource).map_err(SetError::InUse)?;
    }
    SHARED.lock(|s| s.set((offer, s.get().1)));
    CONTROL.signal(offer);
    crate::ww::OBSERVE.mark_changed_plug_pd_source();
    Ok(())
}

/// The setting, as `plug.pd.source` reads.
pub fn setting() -> PdSource {
    SHARED
        .lock(|s| s.get().0)
        .map_or(PdSource::Off, |o| o.setting())
}

/// Where the source stands.
pub fn phase() -> Phase {
    SHARED.lock(|s| s.get().1)
}

/// The source holds plug VBUS: `plug.vbus_mode` and `vbus_cycle` are refused meanwhile.
pub fn holds_vbus() -> bool {
    pins::holds(Holder::PdSource)
}

/// The oldest record not sent yet, with how many were dropped before it.
pub fn take_record() -> Option<(Record, u32)> {
    LOG.lock(|l| l.borrow_mut().pop())
}

fn set_phase(phase: Phase) {
    let changed = SHARED.lock(|s| {
        let (offer, old) = s.get();
        s.set((offer, phase));
        old != phase
    });
    if changed {
        info!("pd: {}", phase);
        crate::ww::OBSERVE.mark_changed_plug_pd_state();
    }
}

fn log(record: Record) {
    LOG.lock(|l| l.borrow_mut().push(record));
    // the server sends it in its next turn
    crate::ww::OBSERVE.wake();
}

fn now_us() -> u64 {
    Instant::now().as_micros()
}

fn log_event(kind: Kind) {
    log(Record::event(now_us(), kind));
}

/// Plug VBUS on in the offer's guarded mode, or off; logged when it changes.
fn vbus(on: Option<&Offer>) {
    let mode = on.map_or(VbusMode::ForceOff, Offer::vbus_mode);
    let was = power::mode(power::Which::Plug) != VbusMode::ForceOff;
    if power::set_mode(power::Which::Plug, mode).is_err() {
        // the receptacle VBUS is over vSafe5V's maximum: the guard keeps it off
        cnt!(pd_vbus_refused: u32, warn);
    }
    crate::ww::OBSERVE.mark_changed_plug_vbus_mode();
    crate::ww::OBSERVE.mark_changed_plug_vbus();
    if was != on.is_some() {
        log_event(Kind::Vbus { on: on.is_some() });
    }
}

static EXECUTOR: InterruptExecutor = InterruptExecutor::new();

/// TIM7's vector, otherwise unused, for [EXECUTOR].
#[interrupt]
unsafe fn TIM7_LPTIM2() {
    // Safety: the vector of this executor, which `start` started
    unsafe { EXECUTOR.on_interrupt() }
}

/// Starts [pd_task] on its own executor: P1, so USB, UCPD, DMA and the time driver (P0, embassy's default on this
/// firmware) still come first, and the thread-mode tasks can't hold it up.
pub fn start(r: PdResources) {
    interrupt::TIM7_LPTIM2.set_priority(Priority::P1);
    EXECUTOR
        .start(interrupt::TIM7_LPTIM2)
        .spawn(defmt::unwrap!(pd_task(r)));
}

/// UCPD2 and its DMA channels, taken by [pd_task].
#[embassy_executor::task]
async fn pd_task(mut r: PdResources) {
    let mut offer: Option<Offer> = None;
    loop {
        let Some(o) = offer else {
            set_phase(Phase::Off);
            pins::release(Holder::PdSource);
            offer = CONTROL.wait().await;
            continue;
        };
        SENDER_RESPONSE_MS.store(o.sender_response_ms as u32, Ordering::Relaxed);
        if o.sender_response_ms != donguru_board::pd::SPEC_SENDER_RESPONSE_MS {
            warn!(
                "pd: SenderResponseTimer {} ms, out of spec (test knob)",
                o.sender_response_ms
            );
        }
        // VBUS off whatever drove it before: an attach starts from vSafe0V
        vbus(None);
        match select(run_attach(&mut r, &o), CONTROL.wait()).await {
            Either::First(()) => {}
            Either::Second(new) => offer = new,
        }
        // embassy's UCPD driver disables its interrupt when the last of its parts drops (after a detach, or the
        // attach cancelled above), and on the G0B1 that vector is USB's too: on again at once, so USB never stops.
        // A USB interrupt raised meanwhile stays pending in the NVIC.
        unsafe { <interrupt::typelevel::USB_UCPD1_2 as interrupt::typelevel::Interrupt>::enable() };
        // after any attach or setting change: off, and off long enough for vSafe0V (UCPD2 already off, its CC
        // open: the sink sees a detach)
        vbus(None);
        if offer.is_some() {
            match select(Timer::after_millis(SRC_RECOVER_MS), CONTROL.wait()).await {
                Either::First(()) => {}
                Either::Second(new) => offer = new,
            }
        }
    }
}

/// One attach: Rp, wait for a sink, VBUS on, the policy engine until the detach.
async fn run_attach(r: &mut PdResources, offer: &Offer) {
    // the CC pins are claimed (`set`), so nothing else drives them; the SWD host keeps them as Flex inputs while
    // it doesn't drive them, so taking them for UCPD2 only switches them to analog (embassy's `Ucpd::new`)
    let (cc1, cc2) = unsafe { (peripherals::PD0::steal(), peripherals::PD2::steal()) };
    let mut ucpd = Ucpd::new(r.ucpd2.reborrow(), cc1, cc2, Irqs, Default::default());
    ucpd.cc_phy().set_pull(match offer.rp() {
        Rp::DefaultUsb => CcPull::SourceDefaultUsb,
        Rp::A1_5 => CcPull::Source1_5A,
        Rp::A3_0 => CcPull::Source3_0A,
    });
    // embassy enables both PHYs: CC2 is the plug's VCONN contact, off (a sink's Rd on its other receptacle pin
    // would otherwise read as a debug accessory)
    embassy_stm32::pac::UCPD2
        .cr()
        .modify(|w| w.set_ccenable(embassy_stm32::pac::ucpd::vals::Ccenable::Cc1));
    set_phase(Phase::Unattached);
    wait_attached(ucpd.cc_phy()).await;
    // the plug's CC is always its CC1 (A5)
    let cc2 = false;
    log_event(Kind::Attached { cc2 });
    set_phase(Phase::Attached { cc2 });
    let (mut cc_phy, pd_phy) =
        ucpd.split_pd_phy(r.rx_dma.reborrow(), r.tx_dma.reborrow(), Irqs, CcSel::Cc1);
    vbus(Some(offer));
    let driver = LoggingDriver {
        phy: pd_phy,
        offer: *offer,
        vbus_settled: false,
    };
    let dpm = Dpm {
        offer: *offer,
        cc2,
        request: None,
    };
    let mut source: Source<_, PdTimer, _> = Source::new(driver, dpm, false);
    match select(source.run(), wait_detached(&mut cc_phy)).await {
        Either::First(Ok(_)) => {
            // a role swap: usbpd answers swaps with Reject here (the DPM's defaults), so this doesn't come
            cnt!(pd_unexpected_exit: u32, warn);
            set_phase(Phase::Failed { cc2 });
        }
        Either::First(Err(_)) => {
            // no answer after the hard resets, or a protocol error: VBUS off until the sink detaches
            cnt!(pd_engine_failed: u32, warn);
            set_phase(Phase::Failed { cc2 });
            vbus(None);
            wait_detached(&mut cc_phy).await;
        }
        Either::Second(()) => {}
    }
    log_event(Kind::Detached);
}

/// A sink's Rd on CC1.
fn rd(cc: &CcPhy<'_, peripherals::UCPD2>) -> bool {
    cc.vstate().0 == CcVState::Low
}

/// Rd on CC1 for tCCDebounce.
async fn wait_attached(cc: &mut CcPhy<'_, peripherals::UCPD2>) {
    loop {
        if !rd(cc) {
            cc.wait_for_vstate_change().await;
            continue;
        }
        // stable for tCCDebounce, or start over
        if with_timeout(
            Duration::from_millis(CC_DEBOUNCE_MS),
            cc.wait_for_vstate_change(),
        )
        .await
        .is_err()
        {
            return;
        }
    }
}

/// CC1 off Rd for tPDDebounce.
async fn wait_detached(cc: &mut CcPhy<'_, peripherals::UCPD2>) {
    loop {
        if rd(cc) {
            cc.wait_for_vstate_change().await;
            continue;
        }
        if with_timeout(
            Duration::from_millis(PD_DEBOUNCE_MS),
            cc.wait_for_vstate_change(),
        )
        .await
        .is_err()
            && !rd(cc)
        {
            return;
        }
    }
}

/// usbpd's timers on embassy-time, the 30 ms ones with the offer's knob (`Offer::timer_ms`).
struct PdTimer;

impl usbpd::timers::Timer for PdTimer {
    async fn after_millis(milliseconds: u64) {
        let ms = if milliseconds == donguru_board::pd::SPEC_SENDER_RESPONSE_MS {
            u64::from(SENDER_RESPONSE_MS.load(Ordering::Relaxed))
        } else {
            milliseconds
        };
        Timer::after_millis(ms).await
    }
}

/// embassy's PD PHY for usbpd, every message and hard reset into the log.
struct LoggingDriver<'d> {
    phy: PdPhy<'d, peripherals::UCPD2>,
    offer: Offer,
    /// VBUS has had its settle time since the attach.
    vbus_settled: bool,
}

impl Driver for LoggingDriver<'_> {
    async fn wait_for_vbus(&mut self) {
        // PE_SRC_Transition_to_default -> PE_SRC_Startup only at vSafe5V (USB PD 3.2 8.3.3.2.9): rev A cannot read
        // plug VBUS (FW-35), so every switch-on gets the settle time. VBUS came on at the attach (`run_attach`); after
        // a hard reset `Dpm::hard_reset` left it off, and without the settle the capabilities went out 3 ms after
        // the switch-on, before U601's soft start was through (FW-60).
        let was_off = power::mode(power::Which::Plug) == VbusMode::ForceOff;
        vbus(Some(&self.offer));
        if was_off || !self.vbus_settled {
            Timer::after_millis(VBUS_SETTLE_MS).await;
            self.vbus_settled = true;
        }
    }

    async fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, DriverRxError> {
        match self.phy.receive(buffer).await {
            Ok(n) => {
                log(Record::message(
                    now_us(),
                    Kind::Rx,
                    &buffer[..n.min(buffer.len())],
                ));
                Ok(n)
            }
            Err(e @ (ucpd::RxError::Crc | ucpd::RxError::Overrun)) => {
                match e {
                    ucpd::RxError::Crc => {
                        cnt!(pd_rx_crc: u32);
                    }
                    _ => {
                        cnt!(pd_rx_overrun: u32);
                    }
                }
                log_event(Kind::RxDiscarded);
                Err(DriverRxError::Discarded)
            }
            Err(ucpd::RxError::HardReset) => {
                log_event(Kind::HardResetRx);
                Err(DriverRxError::HardReset)
            }
        }
    }

    async fn transmit(&mut self, data: &[u8]) -> Result<(), DriverTxError> {
        let r = self.phy.transmit(data).await;
        let outcome = match r {
            Ok(()) => TxOutcome::Sent,
            Err(ucpd::TxError::Discarded) => TxOutcome::Discarded,
            Err(ucpd::TxError::HardReset) => TxOutcome::HardReset,
        };
        log(Record::message(now_us(), Kind::Tx { outcome }, data));
        r.map_err(|e| match e {
            ucpd::TxError::Discarded => DriverTxError::Discarded,
            ucpd::TxError::HardReset => DriverTxError::HardReset,
        })
    }

    async fn transmit_hard_reset(&mut self) -> Result<(), DriverTxError> {
        let r = self.phy.transmit_hardreset().await;
        log_event(Kind::HardResetTx);
        r.map_err(|e| match e {
            ucpd::TxError::Discarded => DriverTxError::Discarded,
            ucpd::TxError::HardReset => DriverTxError::HardReset,
        })
    }
}

/// The device policy manager: offers the one PDO, accepts what fits it, follows the contract in the phase.
struct Dpm {
    offer: Offer,
    cc2: bool,
    /// The Request accepted last, until its `transition_power`.
    request: Option<(u8, u16, u16)>,
}

impl SourceDpm for Dpm {}
impl EprDevicePolicyManager for Dpm {}
impl DrpDevicePolicyManager for Dpm {}

impl DevicePolicyManager for Dpm {
    fn source_capabilities(&mut self) -> SourceCapabilities {
        SourceCapabilities::new_vsafe5v_only(self.offer.max_10ma())
    }

    async fn evaluate_request(&mut self, request: &PowerSource) -> CapabilityResponse {
        if let PowerSource::FixedVariableSupply(rdo) = request
            && self
                .offer
                .accepts(rdo.object_position(), rdo.raw_operating_current())
        {
            self.request = Some((
                rdo.object_position(),
                rdo.raw_operating_current(),
                rdo.raw_max_operating_current(),
            ));
            CapabilityResponse::Accept
        } else {
            cnt!(pd_requests_rejected: u32);
            CapabilityResponse::Reject
        }
    }

    async fn transition_power(&mut self, _power_level: &PowerSource) -> Result<(), ()> {
        // 5 V is all there is, VBUS is there already: nothing to switch
        set_phase(match self.request.take() {
            Some((object_position, operating_10ma, max_10ma)) => Phase::Contract {
                cc2: self.cc2,
                object_position,
                operating_10ma,
                max_10ma,
            },
            // from PE_SRC_Disabled: the capabilities went unanswered nCapsCount times, a sink without PD
            None => Phase::NoPd { cc2: self.cc2 },
        });
        Ok(())
    }

    async fn hard_reset(&mut self) -> Result<(), ()> {
        // vSafe0V for tSrcRecover, then vSafe5V (wait_for_vbus in PE_SRC_Startup turns it on again)
        set_phase(Phase::Attached { cc2: self.cc2 });
        vbus(None);
        Timer::after_millis(SRC_RECOVER_MS).await;
        Ok(())
    }
}
