use std::net::Ipv4Addr;

use super::{
    ChangeRow, CheckLevel, ConfLine, ImportCheck, ImportChecks, LineOutcome, OtherProfile,
    ParsedConf, Pin, Profile, ProfileFacts, ProfileFields, ProfileName, WgConfParser,
};
use crate::list::ListSet;

/// One `.conf` file to preview: `PreviewImport{name, file_name, conf_text}`.
pub struct ImportRequest<'a> {
    pub name: &'a str,
    pub file_name: &'a str,
    pub conf_text: &'a str,
}

/// A stored profile as the preview sees it; `facts` is `None` when its blob cannot be read.
#[derive(Clone, Debug)]
pub struct KnownProfile {
    pub name: ProfileName,
    pub colour: Pin,
    pub facts: Option<ProfileFacts>,
    pub connected: bool,
}

/// What the preview checks against.
pub struct ImportContext<'a> {
    pub profiles: &'a [KnownProfile],
    pub lists: &'a ListSet,
    /// The stored profile of the same name, decrypted, when this is a replace and it could be read.
    pub replaced: Option<&'a Profile>,
    /// The endpoint's addresses as resolved through the fallback; an IP literal needs none.
    pub endpoint_addresses: &'a [Ipv4Addr],
    /// Addresses other profiles' DNS answered recently, `(profile, address)`.
    pub answers: &'a [(String, Ipv4Addr)],
}

/// What importing one `.conf` would do: validation, checks against the other profiles and, for a
/// profile of the same name, the changes. It stores nothing and holds no key material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportPreview {
    pub name: String,
    pub file_name: String,
    /// A profile of this exact name exists: importing replaces its `.conf`.
    pub replace: bool,
    pub name_problem: Option<String>,
    /// For a new profile.
    pub values: Vec<(String, String)>,
    /// For a replace.
    pub changes: Vec<ChangeRow>,
    pub checks: Vec<ImportCheck>,
    pub lines: Vec<ConfLine>,
    pub colour: Pin,
}

impl ImportPreview {
    pub const LIMIT: usize = 16;

    pub fn build(request: &ImportRequest, context: &ImportContext) -> Self {
        Self::of_parsed(request, &WgConfParser::parse(request.conf_text), context)
    }

    pub fn of_parsed(
        request: &ImportRequest,
        parsed: &ParsedConf,
        context: &ImportContext,
    ) -> Self {
        let existing = context
            .profiles
            .iter()
            .find(|p| p.name.same_as(request.name));
        let replaced = existing.filter(|p| p.name.as_str() == request.name);
        let mut preview = Self {
            name: request.name.to_owned(),
            file_name: request.file_name.to_owned(),
            replace: replaced.is_some(),
            name_problem: Self::name_problem(request.name, existing, context.profiles.len()),
            values: Vec::new(),
            changes: Vec::new(),
            checks: Vec::new(),
            lines: parsed.lines.clone(),
            colour: replaced.map_or_else(
                || Pin::first_unused(context.profiles.iter().map(|p| p.colour)),
                |p| p.colour,
            ),
        };
        let Some(conf) = parsed.conf() else {
            return preview;
        };
        let guid = context.replaced.map_or(0, |p| p.facts.guid);
        let new = Profile::from_conf(conf.clone(), guid, 0);
        match replaced {
            Some(old) => {
                let old_facts = context.replaced.map(|p| &p.facts).or(old.facts.as_ref());
                if let Some(old_facts) = old_facts {
                    preview.changes = ProfileFields::changes(old_facts, context.replaced, &new);
                }
            }
            None => preview.values = ProfileFields::values(request.name, &new.facts),
        }
        let checks = ImportChecks {
            others: context
                .profiles
                .iter()
                .filter(|p| !p.name.same_as(request.name))
                .filter_map(|p| {
                    p.facts.as_ref().map(|facts| OtherProfile {
                        name: p.name.as_str(),
                        facts,
                    })
                })
                .collect(),
            lists: context.lists,
            answers: context.answers,
        };
        preview.checks = checks.run(request.name, &new.facts, context.endpoint_addresses);
        if let Some(old) = replaced.filter(|p| p.connected) {
            preview.checks.push(ImportCheck {
                level: CheckLevel::Warn,
                title: "Will reconnect".into(),
                detail: format!(
                    "{} is connected: it disconnects, swaps the keys and connects again",
                    old.name
                ),
            });
        }
        preview
    }

    /// The name, a `.conf` problem or a `bad` check keeps the import from happening.
    pub fn blocks(&self) -> bool {
        !self.problems().is_empty()
    }

    /// Every blocking problem as one line of text.
    pub fn problems(&self) -> Vec<String> {
        let mut problems: Vec<String> = self.name_problem.iter().cloned().collect();
        problems.extend(
            self.lines
                .iter()
                .filter(|l| l.outcome == LineOutcome::Problem)
                .map(|l| match l.line {
                    0 => l.reason.clone(),
                    line => format!("line {line}: {}", l.reason),
                }),
        );
        problems.extend(
            self.checks
                .iter()
                .filter(|c| c.level == CheckLevel::Bad)
                .map(ToString::to_string),
        );
        problems
    }

    fn name_problem(name: &str, existing: Option<&KnownProfile>, count: usize) -> Option<String> {
        if let Err(problem) = ProfileName::parse(name) {
            return Some(problem);
        }
        match existing {
            Some(other) if other.name.as_str() != name => Some(format!(
                "{} already exists; choose another name",
                other.name
            )),
            None if count >= Self::LIMIT => {
                Some(format!("SplitWire holds up to {} profiles", Self::LIMIT))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::fixtures::{
        OTHER_PRIVATE, PRESHARED, PRIVATE, conf, full_conf, sample_profile,
    };
    use crate::profile::{Base64, SecretKey};

    fn known(name: &str, colour: u8, facts: ProfileFacts, connected: bool) -> KnownProfile {
        KnownProfile {
            name: ProfileName::parse(name).unwrap(),
            colour: Pin::new(colour).unwrap(),
            facts: Some(facts),
            connected,
        }
    }

    fn preview(
        name: &str,
        text: &str,
        profiles: &[KnownProfile],
        replaced: Option<&Profile>,
    ) -> ImportPreview {
        let lists = ListSet::default();
        let request = ImportRequest {
            name,
            file_name: &format!("{name}.conf"),
            conf_text: text,
        };
        ImportPreview::build(
            &request,
            &ImportContext {
                profiles,
                lists: &lists,
                replaced,
                endpoint_addresses: &[],
                answers: &[],
            },
        )
    }

    #[test]
    fn a_new_profile_shows_its_values_and_the_first_free_colour() {
        let mut acme = sample_profile().facts;
        acme.address = crate::profile::TunnelAddress::parse("10.1.0.2/32").unwrap();
        let others = [known("AcmeCorp", 1, acme, false)];
        let preview = preview("Partner", &full_conf(), &others, None);
        assert!(!preview.replace);
        assert!(!preview.blocks(), "{:?}", preview.problems());
        assert_eq!(preview.colour.number(), 2);
        assert_eq!(
            preview.values[0],
            ("Adapter".into(), "SplitWire-Partner".into())
        );
        assert!(preview.changes.is_empty());
        assert_eq!(preview.file_name, "Partner.conf");
    }

    #[test]
    fn a_same_named_profile_is_a_replace_with_changes_and_a_reconnect_warning() {
        let stored = sample_profile();
        let others = [known("Contoso", 3, stored.facts.clone(), true)];
        let text = conf(
            OTHER_PRIVATE,
            "10.88.4.12/24",
            "10.66.0.53",
            "vpn.example.com:51820",
        );
        let preview = preview("Contoso", &text, &others, Some(&stored));
        assert!(preview.replace);
        assert_eq!(preview.colour.number(), 3);
        assert!(preview.values.is_empty());
        let fields: Vec<(&str, &str)> = preview
            .changes
            .iter()
            .map(|c| (c.field.as_str(), c.new.as_str()))
            .collect();
        assert_eq!(
            fields,
            [
                ("MTU", "1420"),
                ("DNS", "10.66.0.53"),
                ("Keepalive", "off"),
                ("Preshared key", "none"),
                ("Private key", "changed: yes"),
            ]
        );
        let last = preview.checks.last().unwrap();
        assert_eq!(
            (last.level, last.title.as_str()),
            (CheckLevel::Warn, "Will reconnect")
        );
        assert!(!preview.blocks());
    }

    #[test]
    fn name_problems_block() {
        let acme = sample_profile().facts;
        let others = [known("AcmeCorp", 1, acme.clone(), false)];
        let text = full_conf();
        assert_eq!(
            preview("acmecorp", &text, &others, None)
                .name_problem
                .unwrap(),
            "AcmeCorp already exists; choose another name"
        );
        assert!(preview("Acme Corp (old)", &text, &others, None).blocks());
        assert_eq!(
            preview("CON", &text, &others, None).problems()[0],
            "CON is a reserved Windows name"
        );
        let mut many = Vec::new();
        for index in 0..16u32 {
            let mut facts = acme.clone();
            facts.address =
                crate::profile::TunnelAddress::parse(&format!("10.{index}.0.1/32")).unwrap();
            many.push(known(&format!("P{index}"), 1, facts, false));
        }
        assert_eq!(
            preview("Partner", &text, &many, None).name_problem.unwrap(),
            "SplitWire holds up to 16 profiles"
        );
        assert!(preview("P3", &text, &many, None).name_problem.is_none());
    }

    #[test]
    fn conf_problems_and_overlaps_block() {
        let preview_bad = preview("Partner", "[Interface]\nPrivateKey = x\n", &[], None);
        assert!(preview_bad.blocks());
        assert_eq!(
            preview_bad.problems()[0],
            format!("line 2: {}", Base64::WRONG_LENGTH)
        );
        assert!(preview_bad.values.is_empty());
        let acme = sample_profile().facts;
        let others = [known("AcmeCorp", 1, acme, false)];
        let overlapping = preview("Partner", &full_conf(), &others, None);
        assert!(overlapping.blocks());
        assert!(overlapping.problems()[0].contains("Overlaps AcmeCorp's tunnel 10.88.4.0/24"));
    }

    /// The redaction scan: nothing a preview holds may contain a private or preshared key, in
    /// base64 or as raw bytes.
    #[test]
    fn previews_never_contain_key_material() {
        let stored = sample_profile();
        let others = [known("AcmeCorp", 1, stored.facts.clone(), true)];
        let texts = [
            full_conf(),
            conf(
                OTHER_PRIVATE,
                "10.9.0.2/32",
                "10.9.0.53",
                "198.51.100.9:51820",
            ),
            format!(
                "{PRIVATE}\n[Interface]\nPrivateKey = {PRIVATE}x\n[Peer]\nPresharedKey={PRESHARED}\n"
            ),
        ];
        let mut shown = String::new();
        for text in &texts {
            for name in ["AcmeCorp", "Partner"] {
                let replaced = (name == "AcmeCorp").then_some(&stored);
                let preview = preview(name, text, &others, replaced);
                shown.push_str(&format!("{preview:?}"));
                shown.extend(preview.problems());
            }
        }
        for secret in [PRIVATE, PRESHARED, OTHER_PRIVATE] {
            let key = SecretKey::new(Base64::decode_key(secret).unwrap());
            for window in secret.as_bytes().windows(8) {
                let window = std::str::from_utf8(window).unwrap();
                assert!(
                    !shown.contains(window),
                    "{window} of a key appears in a preview"
                );
            }
            let bytes = key.expose();
            let decimal = format!("{}, {}, {}, {}", bytes[0], bytes[1], bytes[2], bytes[3]);
            assert!(!shown.contains(&decimal));
            assert!(!shown.contains(&String::from_utf8_lossy(&bytes[..6]).into_owned()));
        }
    }
}
