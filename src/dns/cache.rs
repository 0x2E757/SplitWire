use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{Duration, SystemTime};

use super::wire::{DnsResponse, Question};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    name: String,
    qtype: u16,
    qclass: u16,
}

impl CacheKey {
    pub fn of(question: &Question) -> Self {
        Self {
            name: question.name.trim_end_matches('.').to_ascii_lowercase(),
            qtype: question.qtype,
            qclass: question.qclass,
        }
    }
}

/// A cached answer with its TTLs lowered to the time it has left.
#[derive(Clone, Debug)]
pub struct CachedAnswer {
    pub response: DnsResponse,
    pub ips: Vec<Ipv4Addr>,
}

struct Entry {
    response: DnsResponse,
    ips: Vec<Ipv4Addr>,
    expires_at: SystemTime,
}

/// SplitWire's own cache of answers for listed names, so it can be reset together with the
/// learned routes.
pub struct DnsCache {
    entries: HashMap<CacheKey, Entry>,
    capacity: usize,
}

impl DnsCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity,
        }
    }

    pub fn get(&mut self, key: &CacheKey, now: SystemTime) -> Option<CachedAnswer> {
        let entry = self.entries.get(key)?;
        let left = entry
            .expires_at
            .duration_since(now)
            .ok()
            .filter(|left| !left.is_zero());
        let Some(left) = left else {
            self.entries.remove(key);
            return None;
        };
        let mut response = entry.response.clone();
        response.set_ttls(left.as_secs().max(1) as u32);
        Some(CachedAnswer {
            response,
            ips: entry.ips.clone(),
        })
    }

    pub fn put(
        &mut self,
        key: CacheKey,
        response: DnsResponse,
        ips: Vec<Ipv4Addr>,
        ttl: Duration,
        now: SystemTime,
    ) {
        if ttl.is_zero() || self.capacity == 0 {
            return;
        }
        if !self.entries.contains_key(&key) && self.entries.len() >= self.capacity {
            self.evict(now);
        }
        let entry = Entry {
            response,
            ips,
            expires_at: now + ttl,
        };
        self.entries.insert(key, entry);
    }

    pub fn clear(&mut self) -> usize {
        let count = self.entries.len();
        self.entries.clear();
        count
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drops the expired entries, or the one closest to expiry when none has.
    fn evict(&mut self, now: SystemTime) {
        self.entries.retain(|_, e| e.expires_at > now);
        if self.entries.len() >= self.capacity
            && let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.expires_at)
                .map(|(k, _)| k.clone())
        {
            self.entries.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns::ResponseBuilder;
    use crate::dns::wire::{DnsQuery, TYPE_A};
    use std::time::UNIX_EPOCH;

    fn answer(name: &str) -> (CacheKey, DnsResponse) {
        let query = DnsQuery::parse(&DnsQuery::build(1, name, TYPE_A)).unwrap();
        let raw = ResponseBuilder::new(&query)
            .a(name, Ipv4Addr::new(1, 2, 3, 4), 300)
            .build();
        (
            CacheKey::of(&query.question),
            DnsResponse::parse(raw).unwrap(),
        )
    }

    const SEC: Duration = Duration::from_secs(1);

    #[test]
    fn serves_until_the_ttl_with_the_time_left() {
        let mut cache = DnsCache::new(10);
        let (key, response) = answer("a.example.com");
        cache.put(
            key.clone(),
            response,
            vec![Ipv4Addr::new(1, 2, 3, 4)],
            60 * SEC,
            UNIX_EPOCH,
        );
        let hit = cache.get(&key, UNIX_EPOCH + 45 * SEC).unwrap();
        assert_eq!(hit.response.answers()[0].ttl, 15);
        assert_eq!(hit.ips, [Ipv4Addr::new(1, 2, 3, 4)]);
        assert!(cache.get(&key, UNIX_EPOCH + 60 * SEC).is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn keys_ignore_case_and_trailing_dot() {
        let mut cache = DnsCache::new(10);
        let (key, response) = answer("A.Example.com.");
        cache.put(key, response, vec![], 60 * SEC, UNIX_EPOCH);
        let (other, _) = answer("a.example.com");
        assert!(cache.get(&other, UNIX_EPOCH).is_some());
    }

    #[test]
    fn zero_ttl_is_not_cached_and_capacity_is_kept() {
        let mut cache = DnsCache::new(2);
        let (key, response) = answer("zero.example.com");
        cache.put(key, response, vec![], Duration::ZERO, UNIX_EPOCH);
        assert!(cache.is_empty());
        for (i, name) in ["a.com", "b.com", "c.com"].iter().enumerate() {
            let (key, response) = answer(name);
            cache.put(key, response, vec![], (10 + i as u32) * SEC, UNIX_EPOCH);
        }
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&answer("a.com").0, UNIX_EPOCH).is_none());
        assert_eq!(cache.clear(), 2);
    }
}
