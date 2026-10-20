use super::{EngineParts, Inventory};
use crate::alerts::{AlertKind, AlertTexts};
use crate::list::ListSet;

/// Reads the lists again: the skipped lines become `list_problem` alerts, and the
/// connected profiles pick a change up through their statics, the matcher (re-homing learned
/// routes) and NRPT.
pub struct ListsReload<'a> {
    parts: &'a EngineParts,
    inventory: &'a mut Inventory,
}

impl<'a> ListsReload<'a> {
    pub fn new(parts: &'a EngineParts, inventory: &'a mut Inventory) -> Self {
        Self { parts, inventory }
    }

    /// `texts` are `(profile, routes.txt text)` in the user's order.
    pub fn apply(&mut self, texts: &[(String, String)]) {
        let lists = ListSet::load(texts.iter().map(|(p, t)| (p.as_str(), t.as_str())));
        self.raise_problems(&lists);
        let changed = lists != self.inventory.lists;
        self.inventory.lists = lists;
        if !changed || self.parts.active.is_empty() {
            return;
        }
        let now = self.parts.clock.now();
        for profile in self.parts.active.all() {
            let statics: Vec<_> = self
                .inventory
                .lists
                .get(&profile.name)
                .map(|l| l.networks().copied().collect())
                .unwrap_or_default();
            let mut routes = profile.routes.lock().unwrap();
            routes.set_statics(&statics);
            routes.reconcile(now);
        }
        let connected = self.parts.active.names();
        self.parts.rehome(&self.inventory.lists, &connected);
        let _ = self.parts.sync_nrpt(&self.inventory.lists, &connected);
    }

    fn raise_problems(&self, lists: &ListSet) {
        for list in lists.lists() {
            if list.list.problems.is_empty() {
                self.parts
                    .alerts
                    .clear(AlertKind::ListProblem, Some(&list.profile));
            } else {
                self.parts
                    .alerts
                    .raise(AlertTexts::list_problem(&list.profile, &list.list.problems));
            }
        }
    }
}
