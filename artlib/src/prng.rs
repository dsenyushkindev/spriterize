//! A small, self-contained pseudo-random generator for the noise sources.
//!
//! The Python artlib draws its noise lattices from `random.Random(seed)` —
//! CPython's Mersenne Twister. We deliberately do *not* replicate that stream:
//! the port reproduces the noise *types* (value, gradient, cellular, fbm,
//! ridged, stripes), not the exact bytes, because a given seed's precise random
//! values are not something a viewer can tell apart. Reproducing CPython's RNG
//! bit-for-bit would be pure cost.
//!
//! What matters here is only that this is deterministic (a seed always gives the
//! same texture), portable (no platform floats leaking in), and well-distributed
//! enough that sequential seeds — which `fbm` leans on, `seed + octave * 977` —
//! give visibly independent octaves. splitmix64 satisfies all three in a few
//! lines.

/// A seedable stream of uniform values, matching the surface of Python's
/// `random.Random`: the only method the noise sources use is [`Prng::random`].
pub struct Prng {
    state: u64,
}

impl Prng {
    /// Seed the stream. Any seed is valid, including 0 and small sequential
    /// values.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        // splitmix64: increment by the golden-ratio odd constant, then a fixed
        // avalanche. Mixes sequential seeds apart on the very first draw, which
        // is what fbm's `seed + octave*977` relies on.
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A float in `[0, 1)`, the direct analogue of Python's `random.random()`:
    /// 53 bits of mantissa over 2^53, so every representable double in the range
    /// is reachable and none exceeds 1.
    pub fn random(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_deterministic() {
        let mut a = Prng::new(11);
        let mut b = Prng::new(11);
        for _ in 0..1000 {
            assert_eq!(a.random(), b.random());
        }
    }

    #[test]
    fn values_are_in_unit_interval() {
        let mut rng = Prng::new(0);
        for _ in 0..100_000 {
            let v = rng.random();
            assert!((0.0..1.0).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn sequential_seeds_diverge_immediately() {
        // fbm sums octaves seeded seed + o*977; the first draw of each must not
        // be near-identical, or the octaves would stack coherently.
        let first = |seed: u64| Prng::new(seed).random();
        let a = first(7);
        let b = first(7 + 977);
        let c = first(7 + 2 * 977);
        assert!((a - b).abs() > 0.01);
        assert!((b - c).abs() > 0.01);
        assert!((a - c).abs() > 0.01);
    }

    #[test]
    fn mean_is_roughly_half() {
        let mut rng = Prng::new(42);
        let n = 200_000;
        let sum: f64 = (0..n).map(|_| rng.random()).sum();
        let mean = sum / n as f64;
        assert!((mean - 0.5).abs() < 0.01, "mean {mean} not near 0.5");
    }
}
