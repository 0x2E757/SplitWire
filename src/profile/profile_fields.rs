use super::{Profile, ProfileFacts};

/// A changed field of a replaced profile. Keys appear only as `stored`, `present`, `none` and
/// `changed: yes` / `changed: no`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeRow {
    pub field: String,
    pub old: String,
    pub new: String,
}

/// A profile's fields as the import review shows them, and the difference between two profiles.
pub struct ProfileFields;

impl ProfileFields {
    /// `(field, value)` rows of a new profile, without key material.
    pub fn values(name: &str, facts: &ProfileFacts) -> Vec<(String, String)> {
        let mut rows = vec![("Adapter".to_owned(), format!("SplitWire-{name}"))];
        rows.extend(Self::public_rows(facts));
        rows.push((
            "Preshared key".into(),
            Self::presence(facts.has_preshared_key),
        ));
        rows.push(("Private key".into(), "stored encrypted".into()));
        rows
    }

    /// The rows that differ, then the two key rows. `old_keys` is the stored profile when it could
    /// be decrypted; without it the keys count as changed.
    pub fn changes(
        old: &ProfileFacts,
        old_keys: Option<&Profile>,
        new: &Profile,
    ) -> Vec<ChangeRow> {
        let mut rows: Vec<ChangeRow> = Self::public_rows(old)
            .into_iter()
            .zip(Self::public_rows(&new.facts))
            .filter(|((_, before), (_, after))| before != after)
            .map(|((field, old), (_, new))| ChangeRow { field, old, new })
            .collect();
        let changed = |yes: bool| format!("changed: {}", if yes { "yes" } else { "no" });
        let preshared = match (old.has_preshared_key, new.facts.has_preshared_key) {
            (true, true) => changed(old_keys.is_none_or(|k| k.preshared_key_changed(new))),
            (_, present) => Self::presence(present),
        };
        rows.push(ChangeRow {
            field: "Preshared key".into(),
            old: Self::presence(old.has_preshared_key),
            new: preshared,
        });
        rows.push(ChangeRow {
            field: "Private key".into(),
            old: "stored".into(),
            new: changed(old_keys.is_none_or(|k| k.private_key_changed(new))),
        });
        rows
    }

    fn public_rows(facts: &ProfileFacts) -> Vec<(String, String)> {
        let dns: Vec<String> = facts.dns.iter().map(ToString::to_string).collect();
        let keepalive = match facts.keepalive {
            0 => "off".to_owned(),
            secs => format!("{secs} s"),
        };
        [
            ("Endpoint", facts.endpoint.to_string()),
            ("Address", facts.address.to_string()),
            ("MTU", facts.mtu_or_default().to_string()),
            (
                "DNS",
                if dns.is_empty() {
                    "none".into()
                } else {
                    dns.join(", ")
                },
            ),
            ("Peer key", ProfileFacts::short_key(&facts.peer_public_key)),
            ("Keepalive", keepalive),
        ]
        .into_iter()
        .map(|(field, value)| (field.to_owned(), value))
        .collect()
    }

    fn presence(present: bool) -> String {
        if present { "present" } else { "none" }.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::fixtures::sample_profile;
    use crate::profile::{Base64, SecretKey};

    #[test]
    fn values_never_show_keys() {
        let profile = sample_profile();
        let values = ProfileFields::values("AcmeCorp", &profile.facts);
        let shown: Vec<String> = values.iter().map(|(f, v)| format!("{f}: {v}")).collect();
        assert_eq!(
            shown,
            [
                "Adapter: SplitWire-AcmeCorp",
                "Endpoint: vpn.example.com:51820",
                "Address: 10.88.4.12/24",
                "MTU: 1380",
                "DNS: 10.66.0.53, 10.66.0.54",
                "Peer key: xTIBA5rb…mZqp8Dg=",
                "Keepalive: 25 s",
                "Preshared key: present",
                "Private key: stored encrypted",
            ]
        );
    }

    #[test]
    fn changes_list_what_differs_and_keys_only_as_changed() {
        let old = sample_profile();
        let mut new = sample_profile();
        new.facts.mtu = None;
        new.facts.dns.truncate(1);
        assert_eq!(
            ProfileFields::changes(&old.facts, Some(&old), &new),
            [
                ChangeRow {
                    field: "MTU".into(),
                    old: "1380".into(),
                    new: "1420".into()
                },
                ChangeRow {
                    field: "DNS".into(),
                    old: "10.66.0.53, 10.66.0.54".into(),
                    new: "10.66.0.53".into()
                },
                ChangeRow {
                    field: "Preshared key".into(),
                    old: "present".into(),
                    new: "changed: no".into()
                },
                ChangeRow {
                    field: "Private key".into(),
                    old: "stored".into(),
                    new: "changed: no".into()
                },
            ]
        );
        new.private_key = SecretKey::new([9; 32]);
        new.preshared_key = None;
        new.facts.has_preshared_key = false;
        let rows = ProfileFields::changes(&old.facts, Some(&old), &new);
        assert_eq!(rows[rows.len() - 2].new, "none");
        assert_eq!(rows[rows.len() - 1].new, "changed: yes");
        let unknown = ProfileFields::changes(&old.facts, None, &old);
        assert_eq!(unknown[unknown.len() - 1].new, "changed: yes");
        let shown = format!("{rows:?}");
        assert!(!shown.contains(&Base64::encode(old.private_key.expose())[..8]));
    }
}
