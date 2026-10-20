use std::collections::BTreeSet;
use std::io;
use std::sync::Mutex;

use crate::dns::NrptRules;
use crate::list::DomainName;
use crate::logs::Log;

/// Remembers the NRPT rules it was asked for and logs each change; the registry is untouched.
pub struct DryRunNrpt {
    rules: Mutex<BTreeSet<DomainName>>,
    log: Log,
}

impl DryRunNrpt {
    pub fn new(log: Log) -> Self {
        Self {
            rules: Mutex::new(BTreeSet::new()),
            log: log.with_source("nrpt"),
        }
    }

    pub fn rules(&self) -> Vec<String> {
        self.rules
            .lock()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }
}

impl NrptRules for DryRunNrpt {
    fn list(&self) -> io::Result<Vec<DomainName>> {
        Ok(self.rules.lock().unwrap().iter().cloned().collect())
    }

    fn add(&self, domain: &DomainName) -> io::Result<()> {
        self.log.info(format!(
            "dry-run: would add NRPT rule {domain}, .{domain} -> the proxy"
        ));
        self.rules.lock().unwrap().insert(domain.clone());
        Ok(())
    }

    fn remove(&self, domain: &DomainName) -> io::Result<()> {
        self.log
            .info(format!("dry-run: would remove NRPT rule {domain}"));
        self.rules.lock().unwrap().remove(domain);
        Ok(())
    }
}
