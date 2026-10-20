use std::net::Ipv4Addr;

use super::{Base64, Endpoint, SecretKey, TunnelAddress, WgConf};
use crate::tunnel::TunnelSpec;

/// Everything about a profile except its secret keys: what status, previews and checks may use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileFacts {
    /// The adapter GUID, made at import and kept across replaces.
    pub guid: u128,
    pub address: TunnelAddress,
    /// The `.conf` MTU; `profile.conf` `mtu` overrides it.
    pub mtu: Option<u16>,
    pub dns: Vec<Ipv4Addr>,
    pub peer_public_key: [u8; 32],
    pub has_preshared_key: bool,
    pub endpoint: Endpoint,
    pub keepalive: u16,
    /// Unix seconds of the import or the last replace.
    pub imported: u64,
}

impl ProfileFacts {
    pub fn mtu_or_default(&self) -> u16 {
        self.mtu.unwrap_or(TunnelSpec::DEFAULT_MTU)
    }

    /// `hR7mK0vN…xA4dG7k=`: enough to recognise a public key.
    pub fn short_key(key: &[u8; 32]) -> String {
        let text = Base64::encode(key);
        format!("{}…{}", &text[..8], &text[text.len() - 8..])
    }
}

/// A profile with its keys, decrypted from its blob; it lives only while connecting, connected or
/// being stored. The keys are zeroed on drop and redacted in `Debug`.
#[derive(Clone, Debug)]
pub struct Profile {
    pub facts: ProfileFacts,
    pub private_key: SecretKey,
    pub preshared_key: Option<SecretKey>,
}

impl Profile {
    pub fn from_conf(conf: WgConf, guid: u128, imported: u64) -> Self {
        Self {
            facts: ProfileFacts {
                guid,
                address: conf.address,
                mtu: conf.mtu,
                dns: conf.dns,
                peer_public_key: conf.peer_public_key,
                has_preshared_key: conf.preshared_key.is_some(),
                endpoint: conf.endpoint,
                keepalive: conf.keepalive,
                imported,
            },
            private_key: conf.private_key,
            preshared_key: conf.preshared_key,
        }
    }

    /// Whether the private key differs from `other`'s, without showing either.
    pub fn private_key_changed(&self, other: &Profile) -> bool {
        self.private_key.expose() != other.private_key.expose()
    }

    pub fn preshared_key_changed(&self, other: &Profile) -> bool {
        match (&self.preshared_key, &other.preshared_key) {
            (Some(mine), Some(theirs)) => mine.expose() != theirs.expose(),
            (None, None) => false,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_public_keys() {
        let key = Base64::decode_key("hR7mK0vNb2Qe9sTzW1cU4yF6jL8pA3xD5gH0kM2nxA4=").unwrap();
        assert_eq!(ProfileFacts::short_key(&key), "hR7mK0vN…kM2nxA4=");
    }
}
