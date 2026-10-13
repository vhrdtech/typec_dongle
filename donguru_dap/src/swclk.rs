//! SWCLK timing of the firmware's bit-bang SWD kernels (`fw/donguru/src/swd.rs`, FW-20): from what one clock bit
//! costs in CPU cycles, measured on the chip at boot, to the spin counts that give a requested frequency.
//!
//! A kernel bit costs `base + per_spin * (lo + hi)` cycles, `lo` and `hi` being the iterations of the busy-wait in
//! the low and the high half of the clock (each at least 1). The firmware times the kernels with SysTick at two
//! batch sizes (the difference drops the call overhead) and two spin counts (the difference is the spin cost):
//! [`KernelCost::from_samples`].

/// Spin counts for the low and the high half of one SWCLK period in one kernel, both at least 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Halves {
    pub lo: u32,
    pub hi: u32,
}

/// Cycles per bit of one kernel: `base + per_spin * (lo + hi)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KernelCost {
    pub base: u32,
    pub per_spin: u32,
}

impl KernelCost {
    /// Bits per short batch in [`Self::from_samples`]; the long batch is twice as many.
    pub const BATCH: u32 = 16;
    /// Spins per half in the slow samples of [`Self::from_samples`], on top of the fast samples' 1.
    pub const EXTRA: u32 = 8;
    /// Spins the low half gets on top of an even split, times two: 1 = an odd spin goes to the low half. The
    /// kernels (`out_bits` / `in_bits` in the firmware) split their own work so the halves match at `lo == hi`
    /// by the instruction count; the high half also holds the loop's taken branch back to the top, so the odd
    /// spin evens that out.
    pub const LOW_SKEW: u32 = 1;

    /// From four timings in cycles, each including the same fixed call overhead: [`Self::BATCH`] and
    /// 2 x [`Self::BATCH`] bits with 1 spin per half (`fast_*`), and the same with 1 + [`Self::EXTRA`] (`slow_*`).
    pub fn from_samples(fast_short: u32, fast_long: u32, slow_short: u32, slow_long: u32) -> Self {
        let per_bit =
            |short: u32, long: u32| (long.saturating_sub(short) + Self::BATCH / 2) / Self::BATCH;
        let fast = per_bit(fast_short, fast_long);
        let slow = per_bit(slow_short, slow_long);
        let per_spin = ((slow.saturating_sub(fast) + Self::EXTRA) / (2 * Self::EXTRA)).max(1);
        KernelCost {
            base: fast.saturating_sub(2 * per_spin),
            per_spin,
        }
    }

    /// Spin counts for the shortest period not shorter than `period` cycles (`DAP_SWJ_Clock` asks for a maximum
    /// frequency), or the kernel's fastest when it can't go that fast; and the period they give.
    pub fn halves(&self, period: u32) -> (Halves, u32) {
        let per_spin = self.per_spin.max(1);
        let spins = period.saturating_sub(self.base).div_ceil(per_spin).max(2);
        let lo = ((spins + Self::LOW_SKEW) / 2).min(spins - 1);
        (Halves { lo, hi: spins - lo }, self.base + per_spin * spins)
    }
}

/// SPI prescaler setting (`BR`: the clock is `pclk_hz / 2^(br + 1)`, `br` 0..=7) for the fastest SPI clock not
/// above `max_hz`; `None` when even `pclk_hz / 256` is faster.
pub fn spi_prescaler(pclk_hz: u32, max_hz: u32) -> Option<u8> {
    (0..8u8).find(|br| pclk_hz >> (br + 1) <= max_hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The release build's `out_bits`: 11 cycles + 3 per spin, plus 40 cycles of call overhead per sample.
    fn out_kernel() -> KernelCost {
        let t = |bits: u32, spins: u32| 40 + bits * (11 + 3 * 2 * spins);
        KernelCost::from_samples(t(16, 1), t(32, 1), t(16, 9), t(32, 9))
    }

    #[test]
    fn samples_give_the_kernel_cost() {
        assert_eq!(
            out_kernel(),
            KernelCost {
                base: 11,
                per_spin: 3
            }
        );
    }

    #[test]
    fn exact_periods_are_hit() {
        let k = out_kernel();
        // 2 MHz at 64 MHz: 32 cycles = 11 + 3 * 7
        let (h, period) = k.halves(32);
        assert_eq!(period, 32);
        assert_eq!(h.lo + h.hi, 7);
        assert_eq!(h, Halves { lo: 4, hi: 3 });
        // 1 MHz: 64 cycles = 11 + 3 * 17 + 2 -> 18 spins, 65 cycles: never faster than asked
        assert_eq!(k.halves(64).1, 65);
    }

    #[test]
    fn too_fast_is_clamped_to_the_kernel_minimum() {
        let k = out_kernel();
        assert_eq!(k.halves(16), (Halves { lo: 1, hi: 1 }, 17));
        assert_eq!(k.halves(0), (Halves { lo: 1, hi: 1 }, 17));
    }

    #[test]
    fn both_halves_stay_at_least_one() {
        let k = out_kernel();
        for period in 0..10_000 {
            let (h, real) = k.halves(period);
            assert!(h.lo >= 1 && h.hi >= 1, "{period}: {h:?}");
            assert_eq!(real, 11 + 3 * (h.lo + h.hi));
            // at most one spin slower than asked once the request is reachable
            if period >= 17 {
                assert!(real >= period && real - period < 3, "{period}: {real}");
            }
        }
    }

    #[test]
    fn nonsense_samples_do_not_divide_by_zero() {
        let k = KernelCost::from_samples(0, 0, 0, 0);
        assert_eq!(k.per_spin, 1);
        let _ = k.halves(100);
        let zero = KernelCost {
            base: 5,
            per_spin: 0,
        };
        assert_eq!(zero.halves(10).1, 10);
    }

    #[test]
    fn spi_prescaler_never_runs_faster_than_asked() {
        let pclk = 64_000_000;
        assert_eq!(spi_prescaler(pclk, 32_000_000), Some(0));
        assert_eq!(spi_prescaler(pclk, 8_000_000), Some(2));
        assert_eq!(spi_prescaler(pclk, 7_999_999), Some(3));
        assert_eq!(spi_prescaler(pclk, 1_000_000), Some(5));
        assert_eq!(spi_prescaler(pclk, 250_000), Some(7));
        assert_eq!(spi_prescaler(pclk, 249_999), None);
        assert_eq!(spi_prescaler(pclk, u32::MAX), Some(0));
    }
}
