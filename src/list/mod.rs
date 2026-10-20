//! The lists: one list file with its entries and problems, matching names against its domains, and
//! all profiles' lists together (cross-profile duplicates, notes, the combined text, which
//! connected profile a name goes to).

mod combined_text;
mod domain_matcher;
mod domain_name;
mod list_conflicts;
mod list_set;
mod profile_matcher;
mod routes_list;

pub use combined_text::{CombinedListText, SplitText, TextProblem};
pub use domain_matcher::{DomainMatcher, SharedMatcher};
pub use domain_name::DomainName;
pub use list_conflicts::{ListConflicts, ProfileFootprint};
pub use list_set::{CrossDuplicate, ListSet, ProfileList};
pub use profile_matcher::{ProfileMatcher, SharedProfileMatcher};
pub use routes_list::{ListEntry, ListProblem, ListTarget, RoutesList};
