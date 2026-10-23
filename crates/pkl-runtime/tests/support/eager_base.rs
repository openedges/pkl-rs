//! Exact architecture accounting relative to the initialized user module and
//! eager private base graph. This never changes raw Java expectations.
use pkl_runtime::heap::HeapStats;

#[derive(Clone, Copy)]
pub struct Baseline(HeapStats);
impl Baseline {
    pub fn new(initialized: HeapStats) -> Self {
        Self(initialized)
    }
    /// Added user values and custom-this slots, with an independent exact
    /// descriptor/type-node bound. A reclaimed user module contributes -1:
    /// base still owns the canonical Module parent.
    #[track_caller]
    pub fn added_values(self, stats: HeapStats, added_executables: usize) -> isize {
        assert_eq!(
            stats.executable_live,
            self.0.executable_live + added_executables,
            "persistent executable graph above initialized base"
        );
        (stats.live - stats.executable_live) as isize
            - (self.0.live - self.0.executable_live) as isize
    }
}
