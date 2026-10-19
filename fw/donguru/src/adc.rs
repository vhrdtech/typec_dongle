//! Continuous ADC1 sampling into a DMA ring buffer, converted to mV / °C and published via [`latest`].
//!
//! Inputs, selected by [`ChannelSet`]:
//! - [`ChannelSet::Base`]: PA3, PA7, PB0, Vtemp, Vrefint
//! - [`ChannelSet::Extended`]: the above plus PA2, PB1, PB12
//!
//! Averaging is done in hardware (oversampling), and each update uses the newest sequence of a DMA half buffer.
//! Software averaging over the whole half buffer is left commented out in [`run`].
//!
//! Before use the ADC is calibrated [`CAL_ITERATIONS`] times and the averaged calibration factor is written
//! back, as done by `HAL_ADCEx_Calibration_Start` for STM32G0.
//!
//! Conversion (RM0444 "Calculating the actual VDDA voltage using the internal reference voltage"):
//! - VDDA = VREFINT_CAL_VREF * VREFINT_CAL / VREFINT_DATA, VREFINT_CAL from the factory (read at VDDA = 3.0 V)
//! - Vch = VDDA * ADC_DATA / FULL_SCALE
//! - T = (Vsense - V30) / Avg_Slope + 30 °C, V30 = VREFINT_CAL_VREF * TS_CAL1 / FULL_SCALE.
//!   The datasheet only guarantees the single point TS_CAL1, so the slope is the typical Avg_Slope.

use core::cell::Cell;
use core::sync::atomic::{AtomicU32, Ordering};

use defmt::{info, unwrap, warn};
use embassy_futures::select::{Either, select};
use embassy_stm32::adc::{
    Adc, AdcChannel, BorrowedAdcChannel, Clock, Config as AdcConfig, Oversampling,
    OversamplingRatio, Prescaler, SampleTime, VREF_CALIB_MV,
};
use embassy_stm32::peripherals::ADC1;
use embassy_stm32::{pac, rcc};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, block_for};
use static_cell::StaticCell;

use crate::{AdcResources, Irqs};

/// ADC clock = ADCSEL clock (PLL P, 32 MHz, set in main) / PRESC = 32 MHz.
/// Must stay <= [`ADC_CLOCK_MAX_HZ`], checked at startup.
const PRESC: Prescaler = Prescaler::Div1;
const ADC_CLOCK_MAX_HZ: u32 = 35_000_000;
/// Vtemp and Vrefint: >= 5 us (ts_temp) and >= 4 us (ts_vrefint), checked at startup.
/// 160.5 cycles = 5.02 us at 32 MHz. 79.5 cycles only reach 5 us up to 15.9 MHz, and 160.5 (the longest) up to
/// 32.1 MHz, so the ADC clock must not exceed 32.1 MHz.
const SAMPLE_TIME_INTERNAL: SampleTime = SampleTime::Cycles1605;
const INTERNAL_SAMPLE_MIN_NS: u32 = 5_000;
/// External pins. 79.5 cycles = 2.48 us at 32 MHz. 39.5 cycles (1.23 us) was too short for PA7 (button switch,
/// read ~20 mV low), so keep this unless all sources on PA2, PA7, PB1, PB12 are low impedance.
const SAMPLE_TIME_EXTERNAL: SampleTime = SampleTime::Cycles795;
/// Hardware oversampling: 32 conversions summed and shifted right by 1, so each DMA value is the channel
/// average in 1/16 LSB (<= 65520, the 16 bit data register limit), the unit the conversions below work in.
/// Conversion = sample time + 12.5 cycles, so at 32 MHz one oversampled channel takes 173 us (internal) or
/// 92 us (external): one sequence every 622 us (Base, ~1.6 kHz) or 898 us (Extended, ~1.1 kHz).
const OVERSAMPLING: Oversampling = Oversampling::new(OversamplingRatio::X32, 1);
/// Averaged calibration runs.
const CAL_ITERATIONS: u32 = 8;
/// DMA ring buffer length in samples. Every half (120) holds whole sequences of both sets (24 x 5, 15 x 8),
/// giving one [`Readings`] update per ~15 ms (Base) or ~13.5 ms (Extended) at 32 MHz.
const DMA_BUF_LEN: usize = 240;
const HALF_LEN: usize = DMA_BUF_LEN / 2;
const _: () =
    assert!(HALF_LEN.is_multiple_of(BASE.len()) && HALF_LEN.is_multiple_of(EXTENDED.len()));
/// 12 bit.
const FULL_SCALE: u32 = 4095;
/// TS_CAL1: Vtemp raw data at 30 °C, VDDA = VREF_CALIB_MV (DS13560, "Temperature sensor calibration values").
const TS_CAL1_ADDR: *const u16 = 0x1FFF_75A8 as *const u16;
const TS_CAL1_TEMP_CDEG: i32 = 3000;
/// Avg_Slope typ (min 2.3, max 2.7), in uV / °C.
const AVG_SLOPE_UV_PER_C: i32 = 2500;
/// Clamp for the computed VDDA (spec max is 3.6 V). Keeps the integer math below from overflowing.
const VDDA_MAX_MV: u32 = 5000;
/// Bidirectional current sense on PB0: shunt in mOhm, amplifier gain, output referenced to VDDA / 2.
/// I = (V - VDDA / 2) / (gain * shunt), here 1 mV = 4 mA, saturated range about +-6.6A or +-7.2 A if considering 3.6V.
const SHUNT_MOHM: i32 = 5;
const CURRENT_AMP_GAIN: i32 = 50;
/// Voltage divider in front of PA3, Vin = Vpin * (top + bottom) / bottom (x7.17, up to ~25.8 V at 3.6 V).
const PA3_R_TOP_OHM: u32 = 10_000;
const PA3_R_BOT_OHM: u32 = 1_620;
/// One sequence takes this long (see `OVERSAMPLING`), per [`ChannelSet`]: sets how many sequences make a window.
const BASE_SEQUENCE_US: u32 = 622;
const EXTENDED_SEQUENCE_US: u32 = 898;
/// Default averaging window of the meter reading (API-12 `meter.window`).
pub const DEFAULT_WINDOW_US: u32 = 10_000;
#[cfg(feature = "debug-adc")]
const LOG_PERIOD: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum ChannelSet {
    /// PA3, PA7, PB0, Vtemp, Vrefint
    Base,
    /// PA2, PA3, PA7, PB0, PB1, Vtemp, Vrefint, PB12
    Extended,
}

/// Latest converted values, from the newest sequence of a DMA half buffer.
#[derive(Clone, Copy, Default, defmt::Format)]
pub struct Readings {
    pub vdda_mv: u16,
    /// Die temperature in 0.01 °C.
    pub temp_cdeg: i16,
    /// Measured on PA3 at the input of its divider ([`PA3_R_TOP_OHM`], [`PA3_R_BOT_OHM`]).
    pub input_voltage_mv: u16,
    /// PA7
    pub button_switch_mv: u16,
    /// Current through the shunt sensed on PB0 ([`SHUNT_MOHM`], [`CURRENT_AMP_GAIN`]), positive when the
    /// DUT is charged (computer to DUT), negative when the DUT feeds the computer.
    pub input_current_ma: i16,
    /// Highest [`Readings::input_voltage_mv`] of all sequences in the half buffer, for the VBUS guard
    /// (`src/power.rs`): a short rise between two updates isn't missed.
    pub peak_input_voltage_mv: u16,
    /// [`Readings::input_current_ma`] of the biggest magnitude in the half buffer, either sign.
    pub peak_input_current_ma: i16,
    /// PB1. P0, P2 and P3 are `None` with [`ChannelSet::Base`]; P1 is not on an ADC capable pin.
    pub p0_mv: Option<u16>,
    /// PA2
    pub p2_mv: Option<u16>,
    /// PB12
    pub p3_mv: Option<u16>,
    /// The converter codes the values above come from (12 bit, averaged), for `adc.read`'s `raw`.
    pub raw: Raw,
    /// Boot-relative time of the update, ms.
    pub t_ms: u32,
}

/// 12 bit codes, averaged like the values. `p*` are 0 when their channel is not in the sequence.
#[derive(Clone, Copy, Default, defmt::Format)]
pub struct Raw {
    pub vref: u16,
    pub temp: u16,
    pub input_voltage: u16,
    pub button_switch: u16,
    pub input_current: u16,
    pub p0: u16,
    pub p2: u16,
    pub p3: u16,
}

/// Charge, energy and peak current since `since`, integrated over every update (API-12 `meter.totals`).
/// Kept in mC and mJ with the sub-unit remainder in nC / pJ, so no 64-bit division runs in the ADC task.
#[derive(Clone, Copy, Default, defmt::Format)]
pub struct Totals {
    /// Net charge towards the DUT, mC.
    pub charge_mc: i32,
    charge_rem_nc: i32,
    /// Net energy towards the DUT, mJ.
    pub energy_mj: i32,
    energy_rem_pj: i64,
    /// Largest current seen, in magnitude, signed like `input_current_ma`.
    pub peak_ma: i16,
    /// Start of the totals, ms since boot.
    pub since_ms: u32,
}

impl Totals {
    fn add(&mut self, current_ma: i32, power_uw: i32, dt_us: u32) {
        // mA x us = nC, uW x us = pJ
        self.charge_rem_nc += current_ma * dt_us as i32;
        let carry = self.charge_rem_nc / 1_000_000;
        self.charge_rem_nc -= carry * 1_000_000;
        self.charge_mc = self.charge_mc.saturating_add(carry);

        self.energy_rem_pj += power_uw as i64 * dt_us as i64;
        let carry = (self.energy_rem_pj / 1_000_000_000) as i32;
        self.energy_rem_pj -= carry as i64 * 1_000_000_000;
        self.energy_mj = self.energy_mj.saturating_add(carry);

        if current_ma.unsigned_abs() > self.peak_ma.unsigned_abs() as u32 {
            self.peak_ma = current_ma as i16;
        }
    }
}

static TOTALS: Mutex<CriticalSectionRawMutex, Cell<Totals>> = Mutex::new(Cell::new(Totals {
    charge_mc: 0,
    charge_rem_nc: 0,
    energy_mj: 0,
    energy_rem_pj: 0,
    peak_ma: 0,
    since_ms: 0,
}));

/// Totals since boot or the last [`reset_totals`].
pub fn totals() -> Totals {
    TOTALS.lock(Cell::get)
}

/// Starts the totals from zero now.
pub fn reset_totals() {
    let now_ms = Instant::now().as_millis() as u32;
    TOTALS.lock(|t| {
        t.set(Totals {
            since_ms: now_ms,
            ..Totals::default()
        })
    });
}

static WINDOW_US: AtomicU32 = AtomicU32::new(DEFAULT_WINDOW_US);

/// Longest averaging window: one DMA half buffer of the quickest set.
pub const WINDOW_MAX_US: u32 = (HALF_LEN / BASE.len()) as u32 * BASE_SEQUENCE_US;
/// Shortest: one sequence.
pub const WINDOW_MIN_US: u32 = EXTENDED_SEQUENCE_US;

/// Averaging window of [`Readings`], in us; the ADC task takes it at the next update.
pub fn set_window_us(us: u32) {
    WINDOW_US.store(us, Ordering::Relaxed);
}

pub fn window_us() -> u32 {
    WINDOW_US.load(Ordering::Relaxed)
}

static LATEST: Mutex<CriticalSectionRawMutex, Cell<Option<Readings>>> = Mutex::new(Cell::new(None));

/// Most recent readings, `None` until the first DMA half buffer has been converted.
pub fn latest() -> Option<Readings> {
    LATEST.lock(Cell::get)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Input {
    Pa2,
    Pa3,
    Pa7,
    Pb0,
    Pb1,
    Pb12,
    Temp,
    Vrefint,
}

impl Input {
    fn sample_time(self) -> SampleTime {
        match self {
            Input::Temp | Input::Vrefint => SAMPLE_TIME_INTERNAL,
            _ => SAMPLE_TIME_EXTERNAL,
        }
    }
}

/// Sample time in half ADC clock cycles.
fn sample_half_cycles(t: SampleTime) -> u32 {
    match t {
        SampleTime::Cycles15 => 3,
        SampleTime::Cycles35 => 7,
        SampleTime::Cycles75 => 15,
        SampleTime::Cycles125 => 25,
        SampleTime::Cycles195 => 39,
        SampleTime::Cycles395 => 79,
        SampleTime::Cycles795 => 159,
        _ => 321,
    }
}

// With CHSELRMOD = 0 the G0 always scans enabled channels in ascending channel number order (SCANDIR = 0),
// whatever the order passed to embassy, so the lists below are in that order (checked at startup).
/// ADC channels 3, 7, 8, 12, 13
const BASE: [Input; 5] = [
    Input::Pa3,
    Input::Pa7,
    Input::Pb0,
    Input::Temp,
    Input::Vrefint,
];
/// ADC channels 2, 3, 7, 8, 9, 12, 13, 16
const EXTENDED: [Input; 8] = [
    Input::Pa2,
    Input::Pa3,
    Input::Pa7,
    Input::Pb0,
    Input::Pb1,
    Input::Temp,
    Input::Vrefint,
    Input::Pb12,
];

/// Run the ADC calibration [`CAL_ITERATIONS`] times and apply the average of the resulting factors.
/// Needs the regulator on, ADEN = 0 and DMAEN = 0 (see [`disable`]). Leaves the ADC enabled.
fn calibrate() -> u8 {
    let r = pac::ADC1;
    let mut sum = 0u32;
    for _ in 0..CAL_ITERATIONS {
        r.cr().modify(|w| w.set_adcal(true));
        while r.cr().read().adcal() {}
        sum += r.calfact().read().calfact() as u32;
    }
    let factor = ((sum + CAL_ITERATIONS / 2) / CAL_ITERATIONS) as u8;

    // ADEN must not be set within 4 ADC clock cycles after ADCAL clears (64 us at 62.5 kHz, the slowest clock)
    block_for(Duration::from_micros(100));
    // CALFACT can only be written while ADEN = 1 and ADSTART = 0
    r.isr().write(|w| w.set_adrdy(true));
    r.cr().modify(|w| w.set_aden(true));
    while !r.isr().read().adrdy() {}
    r.calfact().write(|w| w.set_calfact(factor));
    factor
}

/// Turns the converter off (ADDIS), keeping the regulator on: `Adc::new_blocking` leaves it enabled, and CCR
/// (VREFEN, TSEN) and the calibration need ADEN = 0. Needs ADSTART = 0.
fn disable() {
    let r = pac::ADC1;
    if r.cr().read().aden() {
        r.cr().modify(|w| w.set_addis(true));
        while r.cr().read().aden() {}
        // G0: 2 ADC kernel clocks between disabling and the next kernel-side step (calibration, CALFACT), else the
        // ADC may freeze (efton.sk STM32 gotcha 221); 1 us covers it at any ADC clock above 2 MHz
        block_for(Duration::from_micros(1));
    }
}

static SWITCH: Signal<CriticalSectionRawMutex, ChannelSet> = Signal::new();

/// Switch the sampled channel set. The ADC is reinitialized and recalibrated, so [`latest`] returns `None`
/// for one DMA half buffer (~15 ms) after the switch. No effect if `set` is already active.
pub fn set_channels(set: ChannelSet) {
    SWITCH.signal(set);
}

#[embassy_executor::task]
pub async fn adc_task(mut r: AdcResources, mut set: ChannelSet) {
    static DMA_BUF: StaticCell<[u16; DMA_BUF_LEN]> = StaticCell::new();
    let dma_buf = DMA_BUF.init([0; DMA_BUF_LEN]);
    // meter.reading samples for hosts observing it in Samples mode (API-39); the task is spawned once
    let mut meter_samples = crate::ww::MeterSamples::take();
    // what observers of meter.reading / temperature / totals were last told (deadband, SRV-25)
    let mut meter_marks = crate::ww::MeterMarks::new();
    loop {
        set = run(&mut r, dma_buf, set, meter_samples.as_mut(), &mut meter_marks).await;
        // run() dropped the ring buffered ADC: ADC stopped and its clock disabled, DMA channel stopped
        LATEST.lock(|l| l.set(None));
    }
}

/// Sample `set` until a different set is requested with [`set_channels`], then return it.
async fn run(
    r: &mut AdcResources,
    dma_buf: &mut [u16; DMA_BUF_LEN],
    set: ChannelSet,
    mut meter_samples: Option<&mut crate::ww::MeterSamples>,
    meter_marks: &mut crate::ww::MeterMarks,
) -> ChannelSet {
    let adc_hz = rcc::frequency::<ADC1>().0 / PRESC.divisor();
    let internal_ns = sample_half_cycles(SAMPLE_TIME_INTERNAL) * 500_000 / (adc_hz / 1000);
    defmt::assert!(
        adc_hz <= ADC_CLOCK_MAX_HZ,
        "ADC clock {} Hz too high, raise PRESC",
        adc_hz
    );
    defmt::assert!(
        internal_ns >= INTERNAL_SAMPLE_MIN_NS,
        "Vtemp/Vrefint sample time {} ns too short, raise SAMPLE_TIME_INTERNAL",
        internal_ns
    );

    // Pins are not switched to analog here: that is the GPIO reset state on G0 (except PA13/PA14)
    let mut config = AdcConfig::default();
    config.clock = Clock::Async(PRESC);
    config.oversampling = Some(OVERSAMPLING);
    // Calibrated once (single run) and enabled: turned off again for the internal channels and our calibration
    let mut adc = Adc::new_blocking(r.adc.reborrow(), config);
    disable();
    // CCR (VREFEN, TSEN) is only writable while ADEN = 0, so before calibrate() enables the ADC
    let mut vrefint = adc.enable_vrefint();
    let vrefint_cal = vrefint.calibrated_value() as u32;
    let mut temp = adc.enable_temperature();
    let ccr = pac::ADC1.ccr().read();
    defmt::assert!(ccr.vrefen() && ccr.tsen(), "ADC: VREFEN/TSEN not set");
    // SAFETY: factory calibration value in system memory, always readable
    let ts_cal1 = unsafe { TS_CAL1_ADDR.read_volatile() } as u32;
    let calfact = calibrate();
    info!(
        "ADC: {}, {} kHz, CALFACT {}, VREFINT_CAL {}, TS_CAL1 {}",
        set,
        adc_hz / 1000,
        calfact,
        vrefint_cal,
        ts_cal1
    );

    let order: &[Input] = match set {
        ChannelSet::Base => &BASE,
        ChannelSet::Extended => &EXTENDED,
    };
    let mut channels: [Option<BorrowedAdcChannel<'_, ADC1>>; 8] = [
        Some(r.pa2.reborrow().degrade_adc()),
        Some(r.pa3.reborrow().degrade_adc()),
        Some(r.pa7.reborrow().degrade_adc()),
        Some(r.pb0.reborrow().degrade_adc()),
        Some(r.pb1.reborrow().degrade_adc()),
        Some(r.pb12.reborrow().degrade_adc()),
        Some(temp.reborrow_adc()),
        Some(vrefint.reborrow_adc()),
    ];
    let mut last_ch = None;
    let mut hw_channels = [0u8; 8];
    let sequence = order.iter().enumerate().map(|(i, &input)| {
        let ch = unwrap!(channels[input as usize].take());
        let hw = ch.get_hw_channel();
        defmt::assert!(
            last_ch < Some(hw),
            "ADC sequence must be in ascending channel order"
        );
        last_ch = Some(hw);
        hw_channels[i] = hw;
        (ch, input.sample_time())
    });

    // Sample times per channel (SMPSEL) and CHSELR with its CCRDY handshake are set up by embassy
    let mut ring = adc.into_ring_buffered(r.dma.reborrow(), dma_buf, Irqs, sequence, None);
    let smpr = pac::ADC1.smpr().read();
    for (&input, &hw) in order.iter().zip(&hw_channels) {
        let slot = smpr.smpsel(hw as usize) as usize;
        defmt::assert!(
            smpr.smp(slot) == input.sample_time(),
            "ADC: channel {} has the wrong sample time",
            hw
        );
    }
    info!("ADC: SMPR {:#010x}", smpr.0);
    ring.start();

    let pos = |input: Input| order.iter().position(|&i| i == input);
    let mut samples = [0u16; HALF_LEN];
    let mut last_update: Option<Instant> = None;
    #[cfg(feature = "debug-adc")]
    let mut next_log = embassy_time::Instant::now();
    loop {
        let res = match select(ring.read(&mut samples), SWITCH.wait()).await {
            Either::First(res) => res,
            Either::Second(new) if new != set => return new,
            // reads restart on a sequence boundary, so dropping the interrupted one is harmless
            Either::Second(_) => continue,
        };
        if res.is_err() {
            // consumer fell behind: drop the stale data, reads stay aligned to whole sequences
            cnt::cnt!(adc_overruns: u32, warn);
            warn!("ADC: DMA ring buffer overrun");
            ring.clear();
            continue;
        }

        // The newest `n` sequences of the half buffer (the meter window, at least one), averaged on top of the
        // hardware oversampling: in 1/16 LSB (<= 65520)
        let sequence_us = match set {
            ChannelSet::Base => BASE_SEQUENCE_US,
            ChannelSet::Extended => EXTENDED_SEQUENCE_US,
        };
        let n = (window_us() / sequence_us).clamp(1, (HALF_LEN / order.len()) as u32);
        let newest = &samples[HALF_LEN - n as usize * order.len()..];
        let avg_x16 = |p: usize| {
            let sum: u32 = newest
                .iter()
                .skip(p)
                .step_by(order.len())
                .map(|&s| s as u32)
                .sum();
            (sum + n / 2) / n
        };

        let vref_x16 = avg_x16(unwrap!(pos(Input::Vrefint))).max(1);
        let vdda_mv =
            ((VREF_CALIB_MV * vrefint_cal * 16 + vref_x16 / 2) / vref_x16).min(VDDA_MAX_MV);
        let to_mv = |p: usize| ((vdda_mv * avg_x16(p) + FULL_SCALE * 8) / (FULL_SCALE * 16)) as u16;
        let mv = |input: Input| pos(input).map(to_mv);
        // Scaled back through the divider from the 1/16 mV pin voltage to keep resolution
        let input_mv = |code_x16: u32| {
            let pa3_pin_x16 = vdda_mv * code_x16 / FULL_SCALE;
            let div = 16 * PA3_R_BOT_OHM;
            ((pa3_pin_x16 * (PA3_R_TOP_OHM + PA3_R_BOT_OHM) + div / 2) / div) as u16
        };
        let pa3 = unwrap!(pos(Input::Pa3));
        let pb0 = unwrap!(pos(Input::Pb0));
        let input_voltage_mv = input_mv(avg_x16(pa3));
        // Offset from the VDDA / 2 reference in codes (ratiometric, so exact whatever VDDA is), then
        // mA = mV * 1000 / (gain * shunt_mOhm), from the 1/16 mV voltage to keep resolution
        // REF minus output: the amplifier output drops below REF when the DUT is charged (B153A INA181 IN+ on the
        // plug side of the shunt), and charging is positive
        let current_ma = |code_x16: u32| {
            let pb0_code_x16 = (FULL_SCALE * 8) as i32 - code_x16 as i32;
            let pb0_x16 = pb0_code_x16 * vdda_mv as i32 / FULL_SCALE as i32;
            let div = 16 * CURRENT_AMP_GAIN * SHUNT_MOHM;
            let half = if pb0_x16 < 0 { -div / 2 } else { div / 2 };
            ((pb0_x16 * 1000 + half) / div) as i16
        };
        let input_current_ma = current_ma(avg_x16(pb0));

        // Worst case of all sequences in the half buffer, for the guard: highest PA3 code, PB0 code furthest from
        // the VDDA / 2 reference (codes only here, one conversion each at the end)
        let mut pa3_max = 0u16;
        let (mut pb0_min, mut pb0_max) = (u16::MAX, 0u16);
        for seq in samples.chunks_exact(order.len()) {
            pa3_max = pa3_max.max(seq[pa3]);
            pb0_min = pb0_min.min(seq[pb0]);
            pb0_max = pb0_max.max(seq[pb0]);
        }
        let peak_input_voltage_mv = input_mv(pa3_max as u32);
        let (i_low, i_high) = (current_ma(pb0_min as u32), current_ma(pb0_max as u32));
        let peak_input_current_ma = if i_low.unsigned_abs() >= i_high.unsigned_abs() {
            i_low
        } else {
            i_high
        };

        // Sensor voltage and V30 in 1/16 mV, then (dV / Avg_Slope) in 0.01 °C
        let vsense_x16 = (vdda_mv * avg_x16(unwrap!(pos(Input::Temp))) / FULL_SCALE) as i32;
        let v30_x16 = (VREF_CALIB_MV * 16 * ts_cal1 / FULL_SCALE) as i32;
        let temp_cdeg =
            TS_CAL1_TEMP_CDEG + (vsense_x16 - v30_x16) * (100 * 1000 / 16) / AVG_SLOPE_UV_PER_C;

        let code = |input: Input| pos(input).map_or(0, |p| ((avg_x16(p) + 8) / 16) as u16);
        let now = Instant::now();
        let now_ms = now.as_millis() as u32;
        let readings = Readings {
            raw: Raw {
                vref: code(Input::Vrefint),
                temp: code(Input::Temp),
                input_voltage: code(Input::Pa3),
                button_switch: code(Input::Pa7),
                input_current: code(Input::Pb0),
                p0: code(Input::Pb1),
                p2: code(Input::Pa2),
                p3: code(Input::Pb12),
            },
            t_ms: now_ms,
            vdda_mv: vdda_mv as u16,
            temp_cdeg: temp_cdeg as i16,
            input_voltage_mv,
            button_switch_mv: unwrap!(mv(Input::Pa7)),
            input_current_ma,
            peak_input_voltage_mv,
            peak_input_current_ma,
            p0_mv: mv(Input::Pb1),
            p2_mv: mv(Input::Pa2),
            p3_mv: mv(Input::Pb12),
        };
        LATEST.lock(|l| l.set(Some(readings)));
        crate::power::on_adc();
        meter_marks.update(&readings, now);

        // meter.reading samples (API-39): every sequence of the half buffer, not averaged, the newest one taken
        // about now (the task's wake-up after the DMA half transfer)
        if let Some(m) = meter_samples.as_mut()
            && m.is_sampling()
        {
            let now_us = now.as_micros() as u32;
            let count = (HALF_LEN / order.len()) as u32;
            for (k, seq) in samples.chunks_exact(order.len()).enumerate() {
                let t_us = now_us.wrapping_sub((count - 1 - k as u32) * sequence_us);
                m.push(
                    t_us,
                    input_mv(seq[pa3] as u32) as i32,
                    current_ma(seq[pb0] as u32) as i32,
                );
            }
        }

        // totals: this update's value over the time since the last one (the first one after a (re)start has no dt)
        if let Some(last) = last_update.replace(now) {
            let dt_us = now.duration_since(last).as_micros().min(100_000) as u32;
            let current_ma = input_current_ma as i32;
            // mV x mA = uW
            let power_uw = input_voltage_mv as i32 * current_ma;
            TOTALS.lock(|t| {
                let mut v = t.get();
                v.add(current_ma, power_uw, dt_us);
                t.set(v);
            });
        }

        #[cfg(feature = "debug-adc")]
        if embassy_time::Instant::now() >= next_log {
            next_log += LOG_PERIOD;
            defmt::debug!("ADC: {}", readings);
        }
    }
}
