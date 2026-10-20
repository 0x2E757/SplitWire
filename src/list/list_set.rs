use super::{ListProblem, ListTarget, RoutesList};

/// An entry of a later profile that an earlier profile already lists: on load the later one loses
/// the line, because a name belongs to one profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrossDuplicate {
    pub profile: String,
    pub line: usize,
    pub other_profile: String,
    pub other_line: usize,
}

/// One profile's list within the set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileList {
    pub profile: String,
    pub list: RoutesList,
}

/// Every profile's list, in the user's order, with the exact duplicates across profiles resolved:
/// the later profile in the order loses the line, and it becomes one of its problems.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListSet {
    lists: Vec<ProfileList>,
    duplicates: Vec<CrossDuplicate>,
}

impl ListSet {
    /// `files` are `(profile, routes.txt text)` in the user's order.
    pub fn load<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut set = Self::default();
        for (profile, text) in files {
            let mut list = RoutesList::parse(text);
            let mut kept = Vec::new();
            for entry in list.entries {
                match set.owner(&entry.target) {
                    Some((other_profile, other_line)) => {
                        let other_profile = other_profile.to_owned();
                        list.problems.push(ListProblem {
                            line: entry.line,
                            text: entry.target.to_string(),
                            reason: Self::also_in(&other_profile, other_line),
                        });
                        set.duplicates.push(CrossDuplicate {
                            profile: profile.to_owned(),
                            line: entry.line,
                            other_profile,
                            other_line,
                        });
                    }
                    None => kept.push(entry),
                }
            }
            list.entries = kept;
            list.problems.sort_by_key(|problem| problem.line);
            set.lists.push(ProfileList {
                profile: profile.to_owned(),
                list,
            });
        }
        set
    }

    pub fn lists(&self) -> &[ProfileList] {
        &self.lists
    }

    pub fn duplicates(&self) -> &[CrossDuplicate] {
        &self.duplicates
    }

    pub fn get(&self, profile: &str) -> Option<&RoutesList> {
        self.lists
            .iter()
            .find(|l| l.profile.eq_ignore_ascii_case(profile))
            .map(|l| &l.list)
    }

    /// The profile and line that list `target`.
    pub fn owner(&self, target: &ListTarget) -> Option<(&str, usize)> {
        self.lists.iter().find_map(|l| {
            l.list
                .entries
                .iter()
                .find(|entry| &entry.target == target)
                .map(|entry| (l.profile.as_str(), entry.line))
        })
    }

    /// Checks an entry typed into `profile`'s add field against every list.
    pub fn check_add(&self, profile: &str, text: &str) -> Result<ListTarget, String> {
        let target = RoutesList::parse_target(text)?;
        match self.owner(&target) {
            Some((owner, line)) if owner.eq_ignore_ascii_case(profile) => {
                Err(format!("already line {line} of {owner}"))
            }
            Some((owner, line)) => Err(Self::also_in(owner, line)),
            None => Ok(target),
        }
    }

    fn also_in(profile: &str, line: usize) -> String {
        format!("also in {profile} line {line}: a name belongs to one profile")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> ListSet {
        ListSet::load([
            ("AcmeCorp", "corp.com\n10.0.0.0/8\n"),
            (
                "Contoso",
                "# contoso\ncontoso.example\nCORP.com\n10.0.0.0/8\n",
            ),
        ])
    }

    #[test]
    fn the_later_profile_loses_an_exact_duplicate() {
        let set = set();
        let contoso = set.get("contoso").unwrap();
        let kept: Vec<String> = contoso
            .entries
            .iter()
            .map(|e| e.target.to_string())
            .collect();
        assert_eq!(kept, ["contoso.example"]);
        let problems: Vec<(usize, &str)> = contoso
            .problems
            .iter()
            .map(|p| (p.line, p.reason.as_str()))
            .collect();
        assert_eq!(
            problems,
            [
                (3, "also in AcmeCorp line 1: a name belongs to one profile"),
                (4, "also in AcmeCorp line 2: a name belongs to one profile"),
            ]
        );
        assert_eq!(set.duplicates().len(), 2);
        assert_eq!(set.duplicates()[0].other_line, 1);
        assert_eq!(set.get("AcmeCorp").unwrap().entries.len(), 2);
    }

    #[test]
    fn checks_an_added_entry_against_every_list() {
        let set = set();
        assert_eq!(
            set.check_add("AcmeCorp", "Corp.com").unwrap_err(),
            "already line 1 of AcmeCorp"
        );
        assert_eq!(
            set.check_add("Contoso", "corp.com").unwrap_err(),
            "also in AcmeCorp line 1: a name belongs to one profile"
        );
        assert_eq!(
            set.check_add("Contoso", "10.0.0.0/8").unwrap_err(),
            "also in AcmeCorp line 2: a name belongs to one profile"
        );
        assert_eq!(
            set.check_add("Contoso", "intranet").unwrap_err(),
            "a domain needs at least 2 labels"
        );
        assert!(set.check_add("Contoso", "a.corp.com").is_ok());
        assert_eq!(
            set.owner(&RoutesList::parse_target("contoso.example").unwrap()),
            Some(("Contoso", 2))
        );
    }
}
