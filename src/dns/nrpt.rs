use std::collections::BTreeSet;
use std::io;
use std::sync::Arc;

use crate::list::DomainName;
use crate::logs::Log;

/// The Name Resolution Policy Table rules SplitWire owns: one per domain, sending the
/// domain and every subdomain to the proxy. Only rules marked as SplitWire's are listed or
/// removed.
pub trait NrptRules: Send + Sync {
    fn list(&self) -> io::Result<Vec<DomainName>>;
    fn add(&self, domain: &DomainName) -> io::Result<()>;
    fn remove(&self, domain: &DomainName) -> io::Result<()>;
}

/// Whether Group Policy defines NRPT rules; Windows then ignores the local ones (read-only).
pub trait NrptPolicyProbe: Send + Sync {
    fn policy_rules_exist(&self) -> io::Result<bool>;
}

/// Keeps SplitWire's NRPT rules equal to the union of the connected profiles' domains, writing
/// only the difference.
pub struct NrptSync {
    rules: Arc<dyn NrptRules>,
    log: Log,
}

impl NrptSync {
    pub fn new(rules: Arc<dyn NrptRules>, log: Log) -> Self {
        Self { rules, log }
    }

    /// Removes the rules not wanted (an earlier run's too) and adds the missing ones. Every
    /// change is tried; the first error is returned.
    pub fn sync(&self, wanted: &[DomainName]) -> io::Result<()> {
        let wanted: BTreeSet<&DomainName> = wanted.iter().collect();
        let present: BTreeSet<DomainName> = self.rules.list()?.into_iter().collect();
        let mut first_error = None;
        for domain in present.iter().filter(|d| !wanted.contains(d)) {
            match self.rules.remove(domain) {
                Ok(()) => self.log.info(format!("NRPT rule removed for {domain}")),
                Err(e) => {
                    self.log
                        .error(format!("removing the NRPT rule for {domain} failed: {e}"));
                    first_error.get_or_insert(e);
                }
            }
        }
        for domain in wanted.into_iter().filter(|d| !present.contains(*d)) {
            match self.rules.add(domain) {
                Ok(()) => self.log.info(format!("NRPT rule added for {domain}")),
                Err(e) => {
                    self.log
                        .error(format!("the NRPT rule for {domain} failed: {e}"));
                    first_error.get_or_insert(e);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Removes every SplitWire rule (every start, and when the proxy stops).
    pub fn remove_all(&self) -> io::Result<()> {
        self.sync(&[])
    }

    pub fn count(&self) -> io::Result<usize> {
        Ok(self.rules.list()?.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use std::sync::Mutex;
    use std::time::UNIX_EPOCH;

    #[derive(Default)]
    struct FakeRules {
        rules: Mutex<BTreeSet<DomainName>>,
        writes: Mutex<u32>,
        refuse: Option<&'static str>,
    }

    impl NrptRules for FakeRules {
        fn list(&self) -> io::Result<Vec<DomainName>> {
            Ok(self.rules.lock().unwrap().iter().cloned().collect())
        }
        fn add(&self, domain: &DomainName) -> io::Result<()> {
            if self.refuse == Some(domain.as_str()) {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
            *self.writes.lock().unwrap() += 1;
            self.rules.lock().unwrap().insert(domain.clone());
            Ok(())
        }
        fn remove(&self, domain: &DomainName) -> io::Result<()> {
            *self.writes.lock().unwrap() += 1;
            self.rules.lock().unwrap().remove(domain);
            Ok(())
        }
    }

    fn domains(names: &[&str]) -> Vec<DomainName> {
        names
            .iter()
            .map(|n| DomainName::parse(n).unwrap())
            .collect()
    }

    fn log() -> Log {
        Log::new(1 << 16, Arc::new(ManualClock::new(UNIX_EPOCH)), false)
    }

    #[test]
    fn follows_the_union_writing_only_the_difference() {
        let rules = Arc::new(FakeRules::default());
        rules.add(&domains(&["stale.com"])[0]).unwrap();
        let log = log();
        let sync = NrptSync::new(rules.clone(), log.clone());

        sync.sync(&domains(&["a.com", "b.com"])).unwrap();
        assert_eq!(rules.list().unwrap(), domains(&["a.com", "b.com"]));
        assert!(log.contains("NRPT rule removed for stale.com"));
        *rules.writes.lock().unwrap() = 0;

        sync.sync(&domains(&["b.com", "c.com"])).unwrap();
        assert_eq!(rules.list().unwrap(), domains(&["b.com", "c.com"]));
        assert_eq!(*rules.writes.lock().unwrap(), 2);
        assert_eq!(sync.count().unwrap(), 2);

        sync.remove_all().unwrap();
        assert!(rules.list().unwrap().is_empty());
    }

    #[test]
    fn a_failed_write_is_reported_after_the_others_are_done() {
        let rules = Arc::new(FakeRules {
            refuse: Some("bad.com"),
            ..FakeRules::default()
        });
        let sync = NrptSync::new(rules.clone(), log());
        let error = sync
            .sync(&domains(&["a.com", "bad.com", "c.com"]))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(rules.list().unwrap(), domains(&["a.com", "c.com"]));
    }
}
