//! Memtest-style integrity check of an SWD link (CORE-5): crafted patterns written to and read back from a
//! target's RAM through the debug probe, so a flaky clock, a bad turnaround, a stuck SBU line or a byte-lane bug in
//! the probe shows up as a mismatch at a known address and bit.
//!
//! The patterns run against any [`WordMemory`], so they are tested here against fake memories with injected
//! faults; [`probe`] (feature `probe-rs`) puts a halted target core behind it. The target's RAM is the medium, the
//! link is what is under test: block patterns go in chunks of [`CHUNK_WORDS`], the way a real probe batches them.

use std::time::{Duration, Instant};

#[cfg(feature = "probe-rs")]
pub mod probe;

/// Words per block read or write.
pub const CHUNK_WORDS: usize = 1024;
/// Mismatches kept per pattern (all are counted).
pub const MAX_KEPT_MISMATCHES: usize = 16;

/// Word-addressed memory, as seen through a probe.
pub trait WordMemory {
    type Error: std::error::Error + Send + Sync + 'static;
    fn read_words(&mut self, addr: u64, buf: &mut [u32]) -> Result<(), Self::Error>;
    fn write_words(&mut self, addr: u64, data: &[u32]) -> Result<(), Self::Error>;
    /// Byte writes, for the byte-lane pattern (8-bit accesses on the bus).
    fn write_bytes(&mut self, addr: u64, data: &[u8]) -> Result<(), Self::Error>;
}

/// The memory under test: `words` 32-bit words from `base` (word-aligned).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub base: u64,
    pub words: usize,
}

impl Region {
    fn addr(&self, index: usize) -> u64 {
        self.base + 4 * index as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pattern {
    /// Walking one and walking zero through all 32 data bits at one address.
    DataBus,
    /// Power-of-two offsets written one by one with an anti-pattern, for stuck or shorted address bits.
    AddressBus,
    /// All zeros, then all ones.
    Solid,
    /// 0x55555555 / 0xAAAAAAAA alternating per word, then inverted.
    Checkerboard,
    /// Each word holds its own address, then its inverse.
    AddressInAddress,
    /// Fill, then up: check and invert, then down: check and invert back (MATS+ per chunk), twice.
    MovingInversions,
    /// Written with byte accesses, read back as words.
    ByteLanes,
    /// Seeded pseudo-random data (xorshift64*), reproducible from the seed.
    Random,
}

impl Pattern {
    pub const ALL: [Pattern; 8] = [
        Pattern::DataBus,
        Pattern::AddressBus,
        Pattern::Solid,
        Pattern::Checkerboard,
        Pattern::AddressInAddress,
        Pattern::MovingInversions,
        Pattern::ByteLanes,
        Pattern::Random,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Pattern::DataBus => "data-bus",
            Pattern::AddressBus => "address-bus",
            Pattern::Solid => "solid",
            Pattern::Checkerboard => "checkerboard",
            Pattern::AddressInAddress => "address-in-address",
            Pattern::MovingInversions => "moving-inversions",
            Pattern::ByteLanes => "byte-lanes",
            Pattern::Random => "random",
        }
    }
}

/// One word that read back wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mismatch {
    pub addr: u64,
    pub expected: u32,
    pub actual: u32,
}

impl Mismatch {
    /// Bits that differ.
    pub fn xor(&self) -> u32 {
        self.expected ^ self.actual
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternResult {
    pub pattern: Pattern,
    /// Words moved over the link, both directions.
    pub words_written: u64,
    pub words_read: u64,
    /// All mismatches counted; the first [`MAX_KEPT_MISMATCHES`] kept.
    pub mismatches: u64,
    pub first_mismatches: Vec<Mismatch>,
    pub elapsed: Duration,
}

impl PatternResult {
    pub fn passed(&self) -> bool {
        self.mismatches == 0
    }
}

/// Runs one pattern over `region`. `seed` only matters for [`Pattern::Random`]. Overwrites the region: saving and
/// restoring it is the caller's job.
pub fn run_pattern<M: WordMemory>(
    mem: &mut M,
    region: Region,
    pattern: Pattern,
    seed: u64,
) -> Result<PatternResult, M::Error> {
    let start = Instant::now();
    let mut t = Tester {
        mem,
        region,
        written: 0,
        read: 0,
        mismatches: 0,
        kept: Vec::new(),
    };
    match pattern {
        Pattern::DataBus => t.data_bus()?,
        Pattern::AddressBus => t.address_bus()?,
        Pattern::Solid => {
            t.fill_verify(|_, _| 0)?;
            t.fill_verify(|_, _| u32::MAX)?;
        }
        Pattern::Checkerboard => {
            t.fill_verify(|i, _| if i % 2 == 0 { 0x5555_5555 } else { 0xAAAA_AAAA })?;
            t.fill_verify(|i, _| if i % 2 == 0 { 0xAAAA_AAAA } else { 0x5555_5555 })?;
        }
        Pattern::AddressInAddress => {
            t.fill_verify(|_, a| a as u32)?;
            t.fill_verify(|_, a| !(a as u32))?;
        }
        Pattern::MovingInversions => {
            t.moving_inversions(0)?;
            t.moving_inversions(0x5555_5555)?;
        }
        Pattern::ByteLanes => t.byte_lanes()?,
        Pattern::Random => {
            let mut fill = XorShift::new(seed);
            t.fill(|_, _| fill.next_u32())?;
            let mut check = XorShift::new(seed);
            t.verify(|_, _| check.next_u32())?;
        }
    }
    Ok(PatternResult {
        pattern,
        words_written: t.written,
        words_read: t.read,
        mismatches: t.mismatches,
        first_mismatches: t.kept,
        elapsed: start.elapsed(),
    })
}

struct Tester<'m, M> {
    mem: &'m mut M,
    region: Region,
    written: u64,
    read: u64,
    mismatches: u64,
    kept: Vec<Mismatch>,
}

impl<M: WordMemory> Tester<'_, M> {
    fn chunks(&self) -> impl DoubleEndedIterator<Item = (usize, usize)> + use<M> {
        let words = self.region.words;
        (0..words.div_ceil(CHUNK_WORDS)).map(move |c| {
            let start = c * CHUNK_WORDS;
            (start, (start + CHUNK_WORDS).min(words))
        })
    }

    fn write(&mut self, index: usize, data: &[u32]) -> Result<(), M::Error> {
        self.written += data.len() as u64;
        self.mem.write_words(self.region.addr(index), data)
    }

    fn check(&mut self, index: usize, expected: &[u32]) -> Result<(), M::Error> {
        let mut buf = vec![0u32; expected.len()];
        self.read += buf.len() as u64;
        self.mem.read_words(self.region.addr(index), &mut buf)?;
        for (k, (&e, &a)) in expected.iter().zip(&buf).enumerate() {
            if e != a {
                self.mismatches += 1;
                if self.kept.len() < MAX_KEPT_MISMATCHES {
                    self.kept.push(Mismatch {
                        addr: self.region.addr(index + k),
                        expected: e,
                        actual: a,
                    });
                }
            }
        }
        Ok(())
    }

    /// `f(index, address)` gives each word's value, called once per word in ascending order.
    fn fill(&mut self, mut f: impl FnMut(usize, u64) -> u32) -> Result<(), M::Error> {
        for (s, e) in self.chunks() {
            let data: Vec<u32> = (s..e).map(|i| f(i, self.region.addr(i))).collect();
            self.write(s, &data)?;
        }
        Ok(())
    }

    fn verify(&mut self, mut f: impl FnMut(usize, u64) -> u32) -> Result<(), M::Error> {
        for (s, e) in self.chunks() {
            let expected: Vec<u32> = (s..e).map(|i| f(i, self.region.addr(i))).collect();
            self.check(s, &expected)?;
        }
        Ok(())
    }

    fn fill_verify(&mut self, f: impl Fn(usize, u64) -> u32) -> Result<(), M::Error> {
        self.fill(&f)?;
        self.verify(&f)
    }

    fn data_bus(&mut self) -> Result<(), M::Error> {
        for bit in 0..32 {
            for v in [1u32 << bit, !(1u32 << bit)] {
                self.write(0, &[v])?;
                self.check(0, &[v])?;
            }
        }
        Ok(())
    }

    /// Barr's address bus test on word offsets 1, 2, 4, ...: a stuck-high, stuck-low or shorted address bit makes
    /// two of these offsets alias.
    fn address_bus(&mut self) -> Result<(), M::Error> {
        const P: u32 = 0xAAAA_AAAA;
        const A: u32 = 0x5555_5555;
        let offsets: Vec<usize> = std::iter::successors(Some(1usize), |o| o.checked_mul(2))
            .take_while(|&o| o < self.region.words)
            .collect();
        for &o in &offsets {
            self.write(o, &[P])?;
        }
        self.write(0, &[A])?;
        for &o in &offsets {
            self.check(o, &[P])?;
        }
        self.write(0, &[P])?;
        for &t in &offsets {
            self.write(t, &[A])?;
            self.check(0, &[P])?;
            for &o in offsets.iter().filter(|&&o| o != t) {
                self.check(o, &[P])?;
            }
            self.write(t, &[P])?;
        }
        Ok(())
    }

    fn moving_inversions(&mut self, p: u32) -> Result<(), M::Error> {
        self.fill(|_, _| p)?;
        let up: Vec<_> = self.chunks().collect();
        for &(s, e) in &up {
            self.check(s, &vec![p; e - s])?;
            self.write(s, &vec![!p; e - s])?;
        }
        for &(s, e) in up.iter().rev() {
            self.check(s, &vec![!p; e - s])?;
            self.write(s, &vec![p; e - s])?;
        }
        self.verify(|_, _| p)
    }

    /// Each word's four bytes differ and carry the word index, written as bytes, read as words.
    fn byte_lanes(&mut self) -> Result<(), M::Error> {
        let value = |i: usize| {
            let b = (i as u32).wrapping_mul(4);
            u32::from_le_bytes([
                b as u8,
                (b + 1) as u8 ^ 0x5A,
                (b + 2) as u8 ^ 0xA5,
                (b + 3) as u8 ^ 0xFF,
            ])
        };
        for (s, e) in self.chunks() {
            let bytes: Vec<u8> = (s..e).flat_map(|i| value(i).to_le_bytes()).collect();
            self.written += (e - s) as u64;
            self.mem.write_bytes(self.region.addr(s), &bytes)?;
        }
        self.verify(|i, _| value(i))
    }
}

/// xorshift64*: small, fast, and the same stream for the same seed on every machine.
struct XorShift(u64);

impl XorShift {
    fn new(seed: u64) -> Self {
        // Zero is a fixed point of xorshift
        XorShift(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    /// RAM with optional faults, addressed from `base`.
    struct FakeRam {
        base: u64,
        words: Vec<u32>,
        /// Bits forced high on every read.
        stuck_high: u32,
        /// Address bit (in words) tied low on every access: aliases the upper half onto the lower.
        address_bit_low: Option<u32>,
        /// Byte writes land in the wrong lane (lanes 1 and 2 swapped).
        swap_lanes: bool,
        /// Every Nth read word comes back with bit 0 flipped.
        flip_every: Option<u64>,
        reads: u64,
    }

    impl FakeRam {
        fn new(base: u64, words: usize) -> Self {
            FakeRam {
                base,
                words: vec![0; words],
                stuck_high: 0,
                address_bit_low: None,
                swap_lanes: false,
                flip_every: None,
                reads: 0,
            }
        }

        fn index(&self, addr: u64) -> usize {
            let mut i = ((addr - self.base) / 4) as usize;
            if let Some(b) = self.address_bit_low {
                i &= !(1usize << b);
            }
            i
        }
    }

    impl WordMemory for FakeRam {
        type Error = Infallible;

        fn read_words(&mut self, addr: u64, buf: &mut [u32]) -> Result<(), Infallible> {
            for (k, w) in buf.iter_mut().enumerate() {
                self.reads += 1;
                *w = self.words[self.index(addr + 4 * k as u64)] | self.stuck_high;
                if self.flip_every.is_some_and(|n| self.reads % n == 0) {
                    *w ^= 1;
                }
            }
            Ok(())
        }

        fn write_words(&mut self, addr: u64, data: &[u32]) -> Result<(), Infallible> {
            for (k, &w) in data.iter().enumerate() {
                let i = self.index(addr + 4 * k as u64);
                self.words[i] = w;
            }
            Ok(())
        }

        fn write_bytes(&mut self, addr: u64, data: &[u8]) -> Result<(), Infallible> {
            for (k, &b) in data.iter().enumerate() {
                let a = addr + k as u64;
                let mut lane = (a % 4) as usize;
                if self.swap_lanes && (lane == 1 || lane == 2) {
                    lane = 3 - lane;
                }
                let i = self.index(a & !3);
                let mut bytes = self.words[i].to_le_bytes();
                bytes[lane] = b;
                self.words[i] = u32::from_le_bytes(bytes);
            }
            Ok(())
        }
    }

    const REGION: Region = Region {
        base: 0x2000_0000,
        words: 3000, // not a multiple of CHUNK_WORDS, nor a power of two
    };

    fn run_all(ram: &mut FakeRam) -> Vec<PatternResult> {
        Pattern::ALL
            .iter()
            .map(|&p| run_pattern(ram, REGION, p, 42).unwrap())
            .collect()
    }

    fn failed(results: &[PatternResult]) -> Vec<&'static str> {
        results
            .iter()
            .filter(|r| !r.passed())
            .map(|r| r.pattern.name())
            .collect()
    }

    #[test]
    fn good_ram_passes_everything() {
        let mut ram = FakeRam::new(REGION.base, REGION.words);
        let results = run_all(&mut ram);
        assert!(failed(&results).is_empty(), "{:?}", failed(&results));
        assert!(
            results
                .iter()
                .all(|r| r.words_read > 0 && r.words_written > 0)
        );
    }

    #[test]
    fn stuck_data_bit_is_caught() {
        let mut ram = FakeRam::new(REGION.base, REGION.words);
        ram.stuck_high = 1 << 17;
        let r = run_pattern(&mut ram, REGION, Pattern::DataBus, 0).unwrap();
        assert!(!r.passed());
        assert!(r.first_mismatches.iter().all(|m| m.xor() == 1 << 17));
        let results = run_all(&mut ram);
        for p in [
            "data-bus",
            "solid",
            "checkerboard",
            "moving-inversions",
            "random",
        ] {
            assert!(failed(&results).contains(&p), "{p} missed a stuck bit");
        }
    }

    #[test]
    fn aliased_address_bit_is_caught() {
        let mut ram = FakeRam::new(REGION.base, REGION.words);
        ram.address_bit_low = Some(9);
        let results = run_all(&mut ram);
        for p in ["address-bus", "address-in-address", "random"] {
            assert!(failed(&results).contains(&p), "{p} missed an address alias");
        }
    }

    #[test]
    fn swapped_byte_lanes_are_caught_only_by_byte_lanes() {
        let mut ram = FakeRam::new(REGION.base, REGION.words);
        ram.swap_lanes = true;
        assert_eq!(failed(&run_all(&mut ram)), ["byte-lanes"]);
    }

    #[test]
    fn rare_read_flip_is_counted_and_located() {
        let mut ram = FakeRam::new(REGION.base, REGION.words);
        ram.flip_every = Some(1000);
        let r = run_pattern(&mut ram, REGION, Pattern::Random, 7).unwrap();
        assert_eq!(r.mismatches, REGION.words as u64 / 1000);
        assert_eq!(r.first_mismatches[0].addr, REGION.base + 4 * 999);
        assert_eq!(r.first_mismatches[0].xor(), 1);
    }

    #[test]
    fn random_is_reproducible_and_seed_dependent() {
        let take = |seed| {
            let mut x = XorShift::new(seed);
            (0..4).map(|_| x.next_u32()).collect::<Vec<_>>()
        };
        assert_eq!(take(1), take(1));
        assert_ne!(take(1), take(2));
        assert_ne!(take(0), [0; 4]);
    }
}
