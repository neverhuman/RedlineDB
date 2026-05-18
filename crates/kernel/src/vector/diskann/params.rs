/// User-facing build configuration (degree, search list size, alpha).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiskAnnParams {
    /// Maximum out-degree per node (`R` in the paper). Standard 64.
    pub max_degree: usize,
    /// Search-list size used during build (`L` in the paper). Should be
    /// >= max_degree; 100 is a typical baseline.
    pub search_list_size: usize,
    /// RobustPrune relaxation factor. 1.0 == strict; 1.2 trades a few
    /// extra edges for substantially better recall.
    pub alpha: f32,
    /// Random seed for the medoid bootstrap and tie-breaks. Tests pin this.
    pub seed: u64,
}

impl Default for DiskAnnParams {
    fn default() -> Self {
        Self {
            max_degree: 64,
            search_list_size: 100,
            alpha: 1.2,
            seed: 0x5EED_D15C_0A99_BABE_u64,
        }
    }
}
