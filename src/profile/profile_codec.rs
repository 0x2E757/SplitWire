use std::collections::HashMap;

use super::{AdapterGuid, Base64, Endpoint, Profile, ProfileFacts, SecretBytes, TunnelAddress};

/// The plaintext inside `profile.dat`: `key=value` lines, version 1. It holds the keys, so it is
/// built in and read from `SecretBytes` only, and its errors never quote a value.
pub struct ProfileCodec;

impl ProfileCodec {
    pub const VERSION: &'static str = "1";
    /// Far more than a profile needs, so the buffer never reallocates while it fills.
    const CAPACITY: usize = 2048;
    const KEYS: [&'static str; 11] = [
        "version",
        "guid",
        "private_key",
        "address",
        "mtu",
        "dns",
        "peer_public_key",
        "preshared_key",
        "endpoint",
        "keepalive",
        "imported",
    ];

    pub fn encode(profile: &Profile) -> SecretBytes {
        let facts = &profile.facts;
        let mut out = SecretBytes::with_capacity(Self::CAPACITY);
        let mut line = |key: &str, value: &str| {
            out.extend(key.as_bytes());
            out.extend(b"=");
            out.extend(value.as_bytes());
            out.extend(b"\n");
        };
        line("version", Self::VERSION);
        line("guid", &AdapterGuid::format(facts.guid));
        line("address", &facts.address.to_string());
        line("mtu", &facts.mtu.map(|m| m.to_string()).unwrap_or_default());
        let dns: Vec<String> = facts.dns.iter().map(ToString::to_string).collect();
        line("dns", &dns.join(","));
        line("peer_public_key", &Base64::encode(&facts.peer_public_key));
        line("endpoint", &facts.endpoint.to_string());
        line("keepalive", &facts.keepalive.to_string());
        line("imported", &facts.imported.to_string());
        out.extend(b"private_key=");
        Base64::encode_secret(profile.private_key.expose(), &mut out);
        out.extend(b"\npreshared_key=");
        if let Some(key) = &profile.preshared_key {
            Base64::encode_secret(key.expose(), &mut out);
        }
        out.extend(b"\n");
        out
    }

    pub fn decode(plaintext: &SecretBytes) -> Result<Profile, String> {
        let text = std::str::from_utf8(plaintext.expose()).map_err(|_| "not UTF-8".to_owned())?;
        let mut values: HashMap<&str, &str> = HashMap::new();
        for (index, line) in text.lines().enumerate() {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("line {}: expected key=value", index + 1))?;
            if !Self::KEYS.contains(&key) {
                return Err(format!("line {}: unknown key {key}", index + 1));
            }
            if values.insert(key, value).is_some() {
                return Err(format!("line {}: {key} appears twice", index + 1));
            }
        }
        if let Some(missing) = Self::KEYS.iter().find(|key| !values.contains_key(*key)) {
            return Err(format!("{missing} is missing"));
        }
        if values["version"] != Self::VERSION {
            return Err("unknown version".into());
        }
        let bad = |key: &str| format!("{key} is not valid");
        let preshared = values["preshared_key"];
        let facts = ProfileFacts {
            guid: AdapterGuid::parse(values["guid"]).ok_or_else(|| bad("guid"))?,
            address: TunnelAddress::parse(values["address"]).map_err(|_| bad("address"))?,
            mtu: Self::optional(values["mtu"]).map_err(|_| bad("mtu"))?,
            dns: Self::list(values["dns"]).map_err(|_| bad("dns"))?,
            peer_public_key: Base64::decode_key(values["peer_public_key"])
                .map_err(|_| bad("peer_public_key"))?,
            has_preshared_key: !preshared.is_empty(),
            endpoint: Endpoint::parse(values["endpoint"]).map_err(|_| bad("endpoint"))?,
            keepalive: values["keepalive"].parse().map_err(|_| bad("keepalive"))?,
            imported: values["imported"].parse().map_err(|_| bad("imported"))?,
        };
        Ok(Profile {
            facts,
            private_key: Base64::decode_secret(values["private_key"])
                .map_err(|_| bad("private_key"))?,
            preshared_key: match preshared {
                "" => None,
                text => Some(Base64::decode_secret(text).map_err(|_| bad("preshared_key"))?),
            },
        })
    }

    fn optional<T: std::str::FromStr>(text: &str) -> Result<Option<T>, T::Err> {
        if text.is_empty() {
            Ok(None)
        } else {
            text.parse().map(Some)
        }
    }

    fn list<T: std::str::FromStr>(text: &str) -> Result<Vec<T>, T::Err> {
        text.split(',')
            .filter(|item| !item.is_empty())
            .map(str::parse)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::SecretKey;
    use crate::profile::fixtures::{PRIVATE, sample_profile as profile};

    #[test]
    fn round_trips_every_field() {
        let original = profile();
        let plaintext = ProfileCodec::encode(&original);
        let decoded = ProfileCodec::decode(&plaintext).unwrap();
        assert_eq!(decoded.facts, original.facts);
        assert!(!decoded.private_key_changed(&original));
        assert!(!decoded.preshared_key_changed(&original));
        let text = std::str::from_utf8(plaintext.expose()).unwrap();
        assert!(text.starts_with("version=1\nguid=6b29fc40-ca47-4067-b31d-00dd010662da\n"));
        assert!(text.contains(&format!("private_key={PRIVATE}\n")));
    }

    #[test]
    fn a_profile_without_preshared_key_or_mtu_round_trips() {
        let mut original = profile();
        original.preshared_key = None;
        original.facts.has_preshared_key = false;
        original.facts.mtu = None;
        original.facts.dns.clear();
        let decoded = ProfileCodec::decode(&ProfileCodec::encode(&original)).unwrap();
        assert_eq!(decoded.facts, original.facts);
        assert!(decoded.preshared_key.is_none());
        let other = Profile {
            private_key: SecretKey::new([1; 32]),
            ..decoded.clone()
        };
        assert!(other.private_key_changed(&decoded));
    }

    #[test]
    fn refuses_damaged_plaintext_without_quoting_keys() {
        let text = std::str::from_utf8(ProfileCodec::encode(&profile()).expose())
            .unwrap()
            .to_owned();
        let broken_key = text.replace(PRIVATE, &PRIVATE[1..]);
        for (damaged, reason) in [
            (text.replace("version=1", "version=2"), "unknown version"),
            (text.replace("keepalive=25\n", ""), "keepalive is missing"),
            (format!("{text}mtu=1\n"), "mtu appears twice"),
            (format!("{text}colour=1\n"), "unknown key colour"),
            (broken_key, "private_key is not valid"),
        ] {
            let error =
                ProfileCodec::decode(&SecretBytes::from_vec(damaged.into_bytes())).unwrap_err();
            assert!(error.contains(reason), "{error}");
            assert!(!error.contains(&PRIVATE[1..9]));
        }
    }
}
