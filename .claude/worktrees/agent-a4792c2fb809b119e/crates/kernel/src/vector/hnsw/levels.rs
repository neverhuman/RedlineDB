//! Level assignment + connectivity rules for HNSW.
//!
//! Following Malkov & Yashunin (2016, §4.1):
//!
//! - Each new node draws a level `l = floor(-ln(uniform()) * mL)`.
//!   `mL = 1 / ln(M)` is the level-multiplier that keeps the expected
//!   neighborhood size across layers constant.
//! - Layer 0 holds *every* node; higher layers hold an exponentially
//!   shrinking subset. The total number of layers is bounded by
//!   `log_M(N)` in expectation.
//! - The neighbor cap per layer is `M` everywhere except layer 0,
//!   which holds `M_max0 = 2 * M` because that level dominates recall
//!   for the final beam search.

/// Tuning knobs for an HNSW index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HnswParams {
    /// Maximum out-degree per node on layers ≥ 1. The standard recommendation
    /// is 16; we expose it so tests can shrink the graph for fast smoke tests.
    pub m: usize,
    /// Maximum out-degree on layer 0. Hard-coded to `2 * m` per the paper.
    pub m_max0: usize,
    /// Beam width during construction. 200 is the paper's recall sweet-spot
    /// for ≥0.95 recall@10 on 100k vectors.
    pub ef_construction: usize,
    /// Default beam width during search. Callers can override per-query.
    pub ef_search: usize,
    /// Vector dimensionality. Insert/search calls that disagree return
    /// `Error::CorruptPage("vector dimension mismatch")`.
    pub dim: usize,
    /// Distance metric. Stored in the meta page so `open()` recovers it.
    pub metric: crate::vector::distance::Metric,
}

impl HnswParams {
    /// Standard recommendation: M=16, ef_construction=200, ef_search=64.
    /// `dim` is supplied by the caller; the metric defaults to L2 because
    /// that's the cheapest kernel and most embedding pipelines normalize
    /// before insertion (cosine ⇔ L2 on normalized vectors).
    pub fn standard(dim: usize) -> Self {
        Self {
            m: 16,
            m_max0: 32,
            ef_construction: 200,
            ef_search: 64,
            dim,
            metric: crate::vector::distance::Metric::L2,
        }
    }

    /// Returns the per-layer neighbor cap. Layer 0 gets `m_max0`, all
    /// higher layers get `m`.
    pub fn neighbor_cap(&self, layer: usize) -> usize {
        if layer == 0 { self.m_max0 } else { self.m }
    }

    /// Validate parameter consistency before allocating an index.
    pub fn validate(&self) -> crate::Result<()> {
        if self.m == 0 {
            return Err(crate::Error::CorruptPage("hnsw m must be > 0"));
        }
        if self.m_max0 < self.m {
            return Err(crate::Error::CorruptPage("hnsw m_max0 must be >= m"));
        }
        if self.ef_construction == 0 || self.ef_search == 0 {
            return Err(crate::Error::CorruptPage("hnsw ef must be > 0"));
        }
        if self.dim == 0 {
            return Err(crate::Error::CorruptPage("hnsw dim must be > 0"));
        }
        Ok(())
    }
}

/// Deterministic xorshift64 PRNG. We avoid pulling in `rand` to keep the
/// kernel's dependency surface minimal — the level draw only needs a
/// uniform source good enough to spread node levels geometrically. Tests
/// that need reproducibility seed the same value.
#[derive(Clone, Copy, Debug)]
pub struct LevelRng {
    state: u64,
}

impl LevelRng {
    pub fn new(seed: u64) -> Self {
        // Avoid the all-zero attractor.
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Uniform sample in (0, 1). The strict-positive lower bound matters:
    /// `-ln(0)` is +∞ and would assign an absurd level. We mask the top
    /// 53 bits and add 1 so the result is `[1, 2^53] / 2^53`, which is
    /// safely positive.
    fn next_uniform_open(&mut self) -> f64 {
        let bits = (self.next_u64() >> 11) | 1;
        bits as f64 / (1_u64 << 53) as f64
    }
}

/// Draw a layer assignment for a new node. `m_l = 1 / ln(M)` is the
/// level-multiplier from the paper. `level = floor(-ln(uniform) * m_l)`
/// produces a geometric distribution where the probability of layer ≥ k
/// is `M^-k`.
pub fn assign_level(rng: &mut LevelRng, m: usize) -> usize {
    let m_l = 1.0_f64 / (m as f64).ln().max(f64::MIN_POSITIVE);
    let u = rng.next_uniform_open();
    (-(u.ln()) * m_l).floor() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbor_cap_layer_0_doubles_m() {
        let params = HnswParams::standard(128);
        assert_eq!(params.neighbor_cap(0), 32);
        assert_eq!(params.neighbor_cap(1), 16);
        assert_eq!(params.neighbor_cap(7), 16);
    }

    #[test]
    fn level_distribution_is_geometric() {
        // P(level ≥ k) ≈ M^-k. Sample 10k draws at M=16 and verify the
        // top-layer counts shrink geometrically. The bound is loose
        // because we only have 10k samples, but a uniform RNG would
        // fail it by orders of magnitude.
        let mut rng = LevelRng::new(0xDEADBEEF);
        let mut counts = [0_usize; 8];
        let trials = 10_000;
        for _ in 0..trials {
            let lvl = assign_level(&mut rng, 16).min(7);
            counts[lvl] += 1;
        }
        // Layer 0 should hold most of the mass.
        assert!(counts[0] > trials * 9 / 10);
        // Each subsequent layer should be at least 4x smaller than the
        // previous *non-empty* layer (paper says 16x; we leave slack for
        // sampling variance at small N).
        let mut prev = counts[0];
        for &c in &counts[1..] {
            if c == 0 {
                break;
            }
            assert!(prev >= c * 4, "non-geometric tail: prev={prev}, c={c}");
            prev = c;
        }
    }

    #[test]
    fn rng_is_deterministic_under_seed() {
        let a = {
            let mut r = LevelRng::new(42);
            (0..16)
                .map(|_| assign_level(&mut r, 16))
                .collect::<Vec<_>>()
        };
        let b = {
            let mut r = LevelRng::new(42);
            (0..16)
                .map(|_| assign_level(&mut r, 16))
                .collect::<Vec<_>>()
        };
        assert_eq!(a, b);
    }

    #[test]
    fn validate_rejects_bad_params() {
        let mut p = HnswParams::standard(128);
        p.m = 0;
        assert!(p.validate().is_err());
        let mut p = HnswParams::standard(128);
        p.dim = 0;
        assert!(p.validate().is_err());
        let mut p = HnswParams::standard(128);
        p.ef_construction = 0;
        assert!(p.validate().is_err());
    }
}
