//! NRPT rules in the registry: no PowerShell, no child process. The root key is a
//! parameter, so the tests work under `HKCU\Software\SplitWire-test` and never touch the real
//! `DnsPolicyConfig`.

use std::io;
use std::net::Ipv4Addr;

use super::registry::{RegistryKey, RegistryRoot};
use crate::dns::{NrptPolicyProbe, NrptRules};
use crate::list::DomainName;

/// `impl NrptRules` over the registry: one key `SplitWire-<domain>` per rule, marked with
/// `Comment=SplitWire`. Only keys with both the prefix and the comment are listed or deleted.
pub struct RegistryNrpt {
    root: RegistryRoot,
    path: String,
    name_server: Ipv4Addr,
}

impl RegistryNrpt {
    pub const LIVE_PATH: &'static str =
        r"SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig";
    pub const PREFIX: &'static str = "SplitWire-";
    pub const COMMENT: &'static str = "SplitWire";
    /// `ConfigOptions`: the generic DNS servers of the rule are used.
    const CONFIG_OPTIONS: u32 = 8;
    const VERSION: u32 = 2;

    /// `name_server` is the proxy's address (`127.0.0.53`).
    pub fn new(root: RegistryRoot, path: &str, name_server: Ipv4Addr) -> Self {
        Self {
            root,
            path: path.to_owned(),
            name_server,
        }
    }

    /// The live rules under HKLM; only a live engine uses it.
    pub fn live(name_server: Ipv4Addr) -> Self {
        Self::new(RegistryRoot::LocalMachine, Self::LIVE_PATH, name_server)
    }

    fn key_name(domain: &DomainName) -> String {
        format!("{}{domain}", Self::PREFIX)
    }

    fn policy(&self, write: bool) -> io::Result<Option<RegistryKey>> {
        RegistryKey::open(self.root, &self.path, write)
    }

    fn ours(parent: &RegistryKey, name: &str) -> io::Result<bool> {
        let Some(key) = parent.open_child(name, false)? else {
            return Ok(false);
        };
        Ok(key.string("Comment")?.as_deref() == Some(Self::COMMENT))
    }
}

impl NrptRules for RegistryNrpt {
    fn list(&self) -> io::Result<Vec<DomainName>> {
        let Some(policy) = self.policy(false)? else {
            return Ok(Vec::new());
        };
        let mut domains = Vec::new();
        for name in policy.subkeys()? {
            let Some(domain) = name.strip_prefix(Self::PREFIX) else {
                continue;
            };
            if let Ok(domain) = DomainName::parse(domain)
                && Self::ours(&policy, &name)?
            {
                domains.push(domain);
            }
        }
        Ok(domains)
    }

    fn add(&self, domain: &DomainName) -> io::Result<()> {
        let policy = RegistryKey::create(self.root, &self.path)?;
        let rule = policy.create_child(&Self::key_name(domain))?;
        rule.set_dword("Version", Self::VERSION)?;
        let dotted = format!(".{domain}");
        rule.set_strings("Name", &[&dotted, domain.as_str()])?;
        rule.set_dword("ConfigOptions", Self::CONFIG_OPTIONS)?;
        rule.set_string("GenericDNSServers", &self.name_server.to_string())?;
        rule.set_string("IPSECCARestriction", "")?;
        // Written last: a rule cut short is not marked as SplitWire's, so it is never listed.
        rule.set_string("Comment", Self::COMMENT)
    }

    fn remove(&self, domain: &DomainName) -> io::Result<()> {
        let Some(policy) = self.policy(true)? else {
            return Ok(());
        };
        let name = Self::key_name(domain);
        if Self::ours(&policy, &name)? {
            policy.delete_tree(&name)?;
        }
        Ok(())
    }
}

/// Group Policy's NRPT rules (read-only): when the policy key has subkeys, Windows ignores the
/// local rules, so listed names are not routed (alert `gp_nrpt`).
pub struct GroupPolicyNrptProbe {
    root: RegistryRoot,
    path: String,
}

impl GroupPolicyNrptProbe {
    pub const LIVE_PATH: &'static str =
        r"SOFTWARE\Policies\Microsoft\Windows NT\DNSClient\DnsPolicyConfig";

    pub fn new(root: RegistryRoot, path: &str) -> Self {
        Self {
            root,
            path: path.to_owned(),
        }
    }

    pub fn live() -> Self {
        Self::new(RegistryRoot::LocalMachine, Self::LIVE_PATH)
    }
}

impl NrptPolicyProbe for GroupPolicyNrptProbe {
    fn policy_rules_exist(&self) -> io::Result<bool> {
        match RegistryKey::open(self.root, &self.path, false)? {
            Some(key) => Ok(!key.subkeys()?.is_empty()),
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::registry::tests::ScratchKey;

    fn domain(name: &str) -> DomainName {
        DomainName::parse(name).unwrap()
    }

    fn open(path: &str) -> RegistryKey {
        RegistryKey::open(RegistryRoot::CurrentUser, path, true)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn writes_lists_and_removes_rules_under_a_scratch_root() {
        let scratch = ScratchKey::new("nrpt");
        let path = format!(r"{}\DnsPolicyConfig", scratch.path);
        let nrpt = RegistryNrpt::new(
            RegistryRoot::CurrentUser,
            &path,
            Ipv4Addr::new(127, 0, 0, 53),
        );
        assert!(nrpt.list().unwrap().is_empty());
        nrpt.add(&domain("corp.example.com")).unwrap();
        nrpt.add(&domain("acme.example")).unwrap();
        let mut listed = nrpt.list().unwrap();
        listed.sort();
        assert_eq!(listed, [domain("acme.example"), domain("corp.example.com")]);

        let rule = open(&format!(r"{path}\SplitWire-corp.example.com"));
        assert_eq!(
            rule.string("GenericDNSServers").unwrap().as_deref(),
            Some("127.0.0.53")
        );
        assert_eq!(
            rule.string("IPSECCARestriction").unwrap().as_deref(),
            Some("")
        );
        assert_eq!(
            rule.string("Comment").unwrap().as_deref(),
            Some("SplitWire")
        );
        drop(rule);

        nrpt.remove(&domain("corp.example.com")).unwrap();
        assert_eq!(nrpt.list().unwrap(), [domain("acme.example")]);
        nrpt.remove(&domain("never.example")).unwrap();
    }

    #[test]
    fn foreign_rules_are_never_listed_or_removed() {
        let scratch = ScratchKey::new("nrpt-foreign");
        let path = format!(r"{}\DnsPolicyConfig", scratch.path);
        let policy = RegistryKey::create(RegistryRoot::CurrentUser, &path).unwrap();
        policy
            .create_child("SplitWire-other.example")
            .unwrap()
            .set_string("Comment", "someone else")
            .unwrap();
        policy
            .create_child("{6B29FC40-CA47-1067-B31D-00DD010662DA}")
            .unwrap()
            .set_string("Comment", "SplitWire")
            .unwrap();
        let nrpt = RegistryNrpt::new(
            RegistryRoot::CurrentUser,
            &path,
            Ipv4Addr::new(127, 0, 0, 53),
        );
        assert!(nrpt.list().unwrap().is_empty());
        nrpt.remove(&domain("other.example")).unwrap();
        assert_eq!(policy.subkeys().unwrap().len(), 2);
    }

    #[test]
    fn the_policy_probe_sees_subkeys_only() {
        let scratch = ScratchKey::new("gp");
        let path = format!(r"{}\DnsPolicyConfig", scratch.path);
        let probe = GroupPolicyNrptProbe::new(RegistryRoot::CurrentUser, &path);
        assert!(!probe.policy_rules_exist().unwrap());
        let policy = RegistryKey::create(RegistryRoot::CurrentUser, &path).unwrap();
        assert!(!probe.policy_rules_exist().unwrap());
        policy.create_child("Rule1").unwrap();
        assert!(probe.policy_rules_exist().unwrap());
    }
}
