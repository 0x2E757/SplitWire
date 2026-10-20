use std::collections::HashMap;
use std::fmt;

use super::DomainName;
use crate::net::Ipv4Net;

/// A line of a list file that was skipped, and why (the wording every surface shows).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListProblem {
    pub line: usize,
    pub text: String,
    pub reason: String,
}

/// What one valid line routes: a domain with every subdomain, or an IPv4 network (an address is a /32).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ListTarget {
    Domain(DomainName),
    Network(Ipv4Net),
}

impl fmt::Display for ListTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListTarget::Domain(domain) => domain.fmt(f),
            ListTarget::Network(net) if net.prefix() == 32 => net.addr().fmt(f),
            ListTarget::Network(net) => net.fmt(f),
        }
    }
}

/// A valid entry and the file line it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListEntry {
    pub line: usize,
    pub target: ListTarget,
}

/// One list file (`routes.txt`) parsed: the valid entries in file order plus the problems of the
/// lines that were skipped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoutesList {
    pub entries: Vec<ListEntry>,
    pub problems: Vec<ListProblem>,
}

impl RoutesList {
    pub const MIN_PREFIX: u8 = 8;
    const NOT_AN_ENTRY: &'static str = "not a domain, IPv4 address or IPv4 network";

    /// One entry per line; `#` starts a comment; blank lines are ignored.
    pub fn parse(text: &str) -> Self {
        let mut list = Self::default();
        let mut seen: HashMap<ListTarget, usize> = HashMap::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let entry_text = raw.split('#').next().unwrap_or_default().trim();
            if entry_text.is_empty() {
                continue;
            }
            let target = match Self::parse_target(entry_text) {
                Ok(target) => target,
                Err(reason) => {
                    list.problem(line, raw, reason);
                    continue;
                }
            };
            if let Some(first) = seen.get(&target) {
                list.problem(line, raw, format!("already line {first}"));
                continue;
            }
            seen.insert(target.clone(), line);
            list.entries.push(ListEntry { line, target });
        }
        list
    }

    /// One entry as typed (an add field checks it before anything else).
    pub fn parse_target(text: &str) -> Result<ListTarget, String> {
        let text = text.trim().to_ascii_lowercase();
        if text.contains(':') {
            return Err("IPv6 is not supported".into());
        }
        if text.contains(char::is_whitespace) {
            return Err(Self::NOT_AN_ENTRY.into());
        }
        if text
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'.' || b == b'/')
        {
            return Self::parse_network(&text).map(ListTarget::Network);
        }
        // A well-formed single label (`intranet`) gets its own wording; other text is not a name.
        let bare = text.trim_end_matches('.');
        if !bare.contains('.') && DomainName::parse(&format!("{bare}.x")).is_ok() {
            return Err("a domain needs at least 2 labels".into());
        }
        DomainName::parse(&text)
            .map(ListTarget::Domain)
            .map_err(|_| Self::NOT_AN_ENTRY.into())
    }

    pub fn domains(&self) -> impl Iterator<Item = &DomainName> {
        self.entries.iter().filter_map(|entry| match &entry.target {
            ListTarget::Domain(domain) => Some(domain),
            ListTarget::Network(_) => None,
        })
    }

    pub fn networks(&self) -> impl Iterator<Item = &Ipv4Net> {
        self.entries.iter().filter_map(|entry| match &entry.target {
            ListTarget::Network(net) => Some(net),
            ListTarget::Domain(_) => None,
        })
    }

    fn parse_network(text: &str) -> Result<Ipv4Net, String> {
        let net: Ipv4Net = text.parse().map_err(|reason: String| {
            if reason.contains("host bits") {
                reason
            } else {
                Self::NOT_AN_ENTRY.into()
            }
        })?;
        if net.prefix() < Self::MIN_PREFIX {
            return Err("prefix shorter than /8".into());
        }
        Ok(net)
    }

    fn problem(&mut self, line: usize, raw: &str, reason: String) {
        self.problems.push(ListProblem {
            line,
            text: raw.trim().to_owned(),
            reason,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(list: &RoutesList) -> Vec<String> {
        list.entries.iter().map(|e| e.target.to_string()).collect()
    }

    #[test]
    fn parses_domains_ips_networks_and_comments() {
        let list = RoutesList::parse(
            "# company\n\nContoso.example\nshop-contoso.example.  # trailing comment\n192.0.2.93\n10.0.0.0/8\n",
        );
        assert_eq!(
            texts(&list),
            [
                "contoso.example",
                "shop-contoso.example",
                "192.0.2.93",
                "10.0.0.0/8"
            ]
        );
        let lines: Vec<usize> = list.entries.iter().map(|e| e.line).collect();
        assert_eq!(lines, [3, 4, 5, 6]);
        let domains: Vec<&str> = list.domains().map(DomainName::as_str).collect();
        assert_eq!(domains, ["contoso.example", "shop-contoso.example"]);
        let nets: Vec<String> = list.networks().map(ToString::to_string).collect();
        assert_eq!(nets, ["192.0.2.93/32", "10.0.0.0/8"]);
        assert!(list.problems.is_empty());
    }

    #[test]
    fn reports_bad_lines_with_numbers_and_keeps_the_rest() {
        let list = RoutesList::parse(
            "good.com\nbad domain.com\n10.0.0.1/8\n::1\n300.1.2.3\ngood.com\nintranet\n10.0.0.0/7\n",
        );
        assert_eq!(texts(&list), ["good.com"]);
        let reasons: Vec<(usize, &str)> = list
            .problems
            .iter()
            .map(|p| (p.line, p.reason.as_str()))
            .collect();
        assert_eq!(
            reasons,
            [
                (2, "not a domain, IPv4 address or IPv4 network"),
                (3, "host bits are set; the network is 10.0.0.0/8"),
                (4, "IPv6 is not supported"),
                (5, "not a domain, IPv4 address or IPv4 network"),
                (6, "already line 1"),
                (7, "a domain needs at least 2 labels"),
                (8, "prefix shorter than /8"),
            ]
        );
        assert_eq!(list.problems[0].text, "bad domain.com");
    }

    #[test]
    fn an_address_and_its_slash_32_are_the_same_entry() {
        let list = RoutesList::parse("192.0.2.93\n192.0.2.93/32\n");
        assert_eq!(list.entries.len(), 1);
        assert_eq!(list.problems[0].reason, "already line 1");
    }

    #[test]
    fn single_entries_are_checked_as_typed() {
        assert_eq!(
            RoutesList::parse_target(" Jira.Example.com "),
            Ok(ListTarget::Domain(
                DomainName::parse("jira.example.com").unwrap()
            ))
        );
        assert!(RoutesList::parse_target("*.example.com").is_err());
        assert_eq!(
            RoutesList::parse_target("2001:db8::/32").unwrap_err(),
            "IPv6 is not supported"
        );
    }

    #[test]
    fn empty_and_comment_only_text_is_an_empty_list() {
        assert_eq!(
            RoutesList::parse("\n   \n# only comments\n"),
            RoutesList::default()
        );
    }
}
