use std::sync::atomic::{AtomicU64, Ordering};

/// How each query that reached the DNS proxy was handled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueryCounters {
    pub total: u64,
    pub listed: u64,
    pub bypassed: u64,
    pub other: u64,
    pub failed: u64,
    pub cache_hits: u64,
}

#[derive(Default)]
pub struct QueryStats {
    total: AtomicU64,
    listed: AtomicU64,
    bypassed: AtomicU64,
    other: AtomicU64,
    failed: AtomicU64,
    cache_hits: AtomicU64,
}

impl QueryStats {
    pub fn count_listed(&self) {
        Self::bump(&self.total);
        Self::bump(&self.listed);
    }

    pub fn count_bypassed(&self) {
        Self::bump(&self.total);
        Self::bump(&self.bypassed);
    }

    pub fn count_other(&self) {
        Self::bump(&self.total);
        Self::bump(&self.other);
    }

    pub fn count_failed(&self) {
        Self::bump(&self.failed);
    }

    pub fn count_cache_hit(&self) {
        Self::bump(&self.cache_hits);
    }

    pub fn snapshot(&self) -> QueryCounters {
        QueryCounters {
            total: self.total.load(Ordering::Relaxed),
            listed: self.listed.load(Ordering::Relaxed),
            bypassed: self.bypassed.load(Ordering::Relaxed),
            other: self.other.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
        }
    }

    fn bump(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_also_counts_towards_the_total() {
        let stats = QueryStats::default();
        stats.count_listed();
        stats.count_bypassed();
        stats.count_other();
        stats.count_failed();
        stats.count_cache_hit();
        let counters = stats.snapshot();
        assert_eq!(counters.total, 3);
        assert_eq!(
            (counters.listed, counters.bypassed, counters.other),
            (1, 1, 1)
        );
        assert_eq!((counters.failed, counters.cache_hits), (1, 1));
    }
}
