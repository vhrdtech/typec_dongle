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

use defmt::{info, unwrap, warn};
use embassy_futures::select::{Either, select};
use embassy_stm32::adc::vals::Smpsel;
use embassy_stm32::adc::{
    Adc, AdcChannel, AdcConfig, AnyAdcChannel, CONTINUOUS, Clock, Exten, Ovsr, Ovss, Presc,
    SampleTime, VREF_CALIB_MV,
};
use embassy_stm32::peripherals::ADC1;
use embassy_stm32::{pac, rcc};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, block_for};
use static_cell::StaticCell;

use crate::{AdcResources, Irqs};

/// ADC clock = ADCSEL clock (PLL P, 32 MHz, set in main) / PRESC = 32 MHz.
/// Must stay <= [`ADC_CLOCK_MAX_HZ`], checked at startup.
const PRESC: Presc = Presc::DIV1;
const ADC_CLOCK_MAX_HZ: u32 = 35_000_000;
/// Vtemp and Vrefint: >= 5 us (ts_temp) and >= 4 us (ts_vrefint), checked at startup.
/// 160.5 cycles = 5.02 us at 32 MHz. 79.5 cycles only reach 5 us up to 15.9 MHz, and 160.5 (the longest) up to
/// 32.1 MHz, so the ADC clock must not exceed 32.1 MHz.
const SAMPLE_TIME_INTERNAL: SampleTime = SampleTime::CYCLES160_5;
const INTERNAL_SAMPLE_MIN_NS: u32 = 5_000;
/// External pins. 79.5 cycles = 2.48 us at 32 MHz. 39.5 cycles (1.23 us) was too short for PA7 (button switch,
/// read ~20 mV low), so keep this unless all sources on PA2, PA7, PB1, PB12 are low impedance.
const SAMPLE_TIME_EXTERNAL: SampleTime = SampleTime::CYCLES79_5;
/// Hardware oversampling: 32 conversions summed and shifted right by 1, so each DMA value is the channel
/// average in 1/16 LSB (<= 65520, the 16 bit data register limit), the unit the conversions below work in.
/// Conversion = sample time + 12.5 cycles, so at 32 MHz one oversampled channel takes 173 us (internal) or
/// 92 us (external): one sequence every 622 us (Base, ~1.6 kHz) or 898 us (Extended, ~1.1 kHz).
const OVS_RATIO: Ovsr = Ovsr::MUL32;
const OVS_SHIFT: Ovss = Ovss::SHIFT1;
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
    /// PB1. P0, P2 and P3 are `None` with [`ChannelSet::Base`]; P1 is not on an ADC capable pin.
    pub p0_mv: Option<u16>,
    /// PA2
    pub p2_mv: Option<u16>,
    /// PB12
    pub p3_mv: Option<u16>,
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
        SampleTime::CYCLES1_5 => 3,
        SampleTime::CYCLES3_5 => 7,
        SampleTime::CYCLES7_5 => 15,
        SampleTime::CYCLES12_5 => 25,
        SampleTime::CYCLES19_5 => 39,
        SampleTime::CYCLES39_5 => 79,
        SampleTime::CYCLES79_5 => 159,
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
/// Needs the regulator on, ADEN = 0 and DMAEN = 0 (true after `Adc::new*`). Leaves the ADC enabled.
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

fn presc_div(p: Presc) -> u32 {
    match p {
        Presc::DIV1 => 1,
        Presc::DIV2 => 2,
        Presc::DIV4 => 4,
        Presc::DIV6 => 6,
        Presc::DIV8 => 8,
        Presc::DIV10 => 10,
        Presc::DIV12 => 12,
        Presc::DIV16 => 16,
        Presc::DIV32 => 32,
        Presc::DIV64 => 64,
        Presc::DIV128 => 128,
        _ => 256,
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
    loop {
        set = run(&mut r, dma_buf, set).await;
        // run() dropped the ring buffered ADC: ADC stopped and its clock disabled, DMA channel stopped
        LATEST.lock(|l| l.set(None));
    }
}

/// Sample `set` until a different set is requested with [`set_channels`], then return it.
async fn run(
    r: &mut AdcResources,
    dma_buf: &mut [u16; DMA_BUF_LEN],
    set: ChannelSet,
) -> ChannelSet {
    let adc_hz = rcc::frequency::<ADC1>().0 / presc_div(PRESC);
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
    let adc = Adc::new_with_config(
        r.adc.reborrow(),
        AdcConfig {
            clock: Some(Clock::Async { div: PRESC }),
            oversampling_ratio: Some(OVS_RATIO),
            oversampling_shift: Some(OVS_SHIFT),
            oversampling_enable: Some(true),
            ..Default::default()
        },
    );
    // CCR (VREFEN, TSEN) is only writable while ADEN = 0, so before calibrate() enables the ADC
    let vrefint = adc.enable_vrefint();
    let vrefint_cal = vrefint.calibrated_value() as u32;
    let temp = adc.enable_temperature();
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
    let mut channels: [Option<AnyAdcChannel<'_, ADC1>>; 8] = [
        Some(r.pa2.reborrow().degrade_adc()),
        Some(r.pa3.reborrow().degrade_adc()),
        Some(r.pa7.reborrow().degrade_adc()),
        Some(r.pb0.reborrow().degrade_adc()),
        Some(r.pb1.reborrow().degrade_adc()),
        Some(r.pb12.reborrow().degrade_adc()),
        Some(temp.degrade_adc()),
        Some(vrefint.degrade_adc()),
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

    let mut ring = adc.into_ring_buffered(
        r.dma.reborrow(),
        dma_buf,
        Irqs,
        sequence,
        CONTINUOUS,
        Exten::DISABLED,
    );
    // embassy 0.6 does not set SMPSEL for the first channel using each sample time, leaving it on SMP1
    // (Vtemp ended up with the external sample time), so program SMPR here. Allowed while ADSTART = 0.
    pac::ADC1.smpr().write(|w| {
        w.set_sample_time(0, SAMPLE_TIME_EXTERNAL);
        w.set_sample_time(1, SAMPLE_TIME_INTERNAL);
        for (&input, &hw) in order.iter().zip(&hw_channels) {
            if input.sample_time() == SAMPLE_TIME_INTERNAL {
                w.set_smpsel(hw as usize, Smpsel::SMP2);
            }
        }
    });
    // After a CHSELR write (CHSELRMOD = 0) conversions must not start before CCRDY. into_ring_buffered() may
    // already have cleared that flag (it clears OVR with a read-modify-write of the write-1-to-clear ISR), so
    // clear it and write CHSELR again to get a CCRDY we can wait for
    let isr = pac::ADC1.isr();
    isr.write(|w| w.set_ccrdy(true));
    let chselr = pac::ADC1.chselr();
    chselr.write_value(chselr.read());
    let mut spins = 0u32;
    while !isr.read().ccrdy() && spins < 100_000 {
        spins += 1;
    }
    if !isr.read().ccrdy() {
        warn!("ADC: CCRDY timeout");
    }
    isr.write(|w| w.set_ccrdy(true));
    ring.start();

    let pos = |input: Input| order.iter().position(|&i| i == input);
    let mut samples = [0u16; HALF_LEN];
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

        // Newest sequence, averaged by the hardware oversampling: already in 1/16 LSB (<= 65520)
        let newest = &samples[HALF_LEN - order.len()..];
        let avg_x16 = |p: usize| newest[p] as u32;
        // Software averaging over all sequences of the half buffer, on top of the hardware oversampling:
        // let n = (HALF_LEN / order.len()) as u32;
        // let avg_x16 = |p: usize| {
        //     let sum: u32 = samples
        //         .iter()
        //         .skip(p)
        //         .step_by(order.len())
        //         .map(|&s| s as u32)
        //         .sum();
        //     (sum + n / 2) / n
        // };

        let vref_x16 = avg_x16(unwrap!(pos(Input::Vrefint))).max(1);
        let vdda_mv =
            ((VREF_CALIB_MV * vrefint_cal * 16 + vref_x16 / 2) / vref_x16).min(VDDA_MAX_MV);
        let to_mv = |p: usize| ((vdda_mv * avg_x16(p) + FULL_SCALE * 8) / (FULL_SCALE * 16)) as u16;
        let mv = |input: Input| pos(input).map(to_mv);
        // Scaled back through the divider from the 1/16 mV pin voltage to keep resolution
        let pa3_pin_x16 = vdda_mv * avg_x16(unwrap!(pos(Input::Pa3))) / FULL_SCALE;
        let div = 16 * PA3_R_BOT_OHM;
        let input_voltage_mv =
            ((pa3_pin_x16 * (PA3_R_TOP_OHM + PA3_R_BOT_OHM) + div / 2) / div) as u16;
        // Offset from the VDDA / 2 reference in codes (ratiometric, so exact whatever VDDA is), then
        // mA = mV * 1000 / (gain * shunt_mOhm), from the 1/16 mV voltage to keep resolution
        // REF minus output: the amplifier output drops below REF when the DUT is charged (B153A INA181 IN+ on the
        // plug side of the shunt), and charging is positive
        let pb0_code_x16 = (FULL_SCALE * 8) as i32 - avg_x16(unwrap!(pos(Input::Pb0))) as i32;
        let pb0_x16 = pb0_code_x16 * vdda_mv as i32 / FULL_SCALE as i32;
        let div = 16 * CURRENT_AMP_GAIN * SHUNT_MOHM;
        let half = if pb0_x16 < 0 { -div / 2 } else { div / 2 };
        let input_current_ma = ((pb0_x16 * 1000 + half) / div) as i16;

        // Sensor voltage and V30 in 1/16 mV, then (dV / Avg_Slope) in 0.01 °C
        let vsense_x16 = (vdda_mv * avg_x16(unwrap!(pos(Input::Temp))) / FULL_SCALE) as i32;
        let v30_x16 = (VREF_CALIB_MV * 16 * ts_cal1 / FULL_SCALE) as i32;
        let temp_cdeg =
            TS_CAL1_TEMP_CDEG + (vsense_x16 - v30_x16) * (100 * 1000 / 16) / AVG_SLOPE_UV_PER_C;

        let readings = Readings {
            vdda_mv: vdda_mv as u16,
            temp_cdeg: temp_cdeg as i16,
            input_voltage_mv,
            button_switch_mv: unwrap!(mv(Input::Pa7)),
            input_current_ma,
            p0_mv: mv(Input::Pb1),
            p2_mv: mv(Input::Pa2),
            p3_mv: mv(Input::Pb12),
        };
        LATEST.lock(|l| l.set(Some(readings)));

        #[cfg(feature = "debug-adc")]
        if embassy_time::Instant::now() >= next_log {
            next_log += LOG_PERIOD;
            defmt::debug!("ADC: {}", readings);
        }
    }
}
