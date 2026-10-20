use super::domain_wire::DomainWire;
use super::{Reader, WireError, Writer};
use crate::list::TextProblem;

/// What one line of a list file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListLineKind {
    Domain,
    Address,
    Network,
    Comment,
    Blank,
    /// Skipped: not a valid entry, or an entry another profile owns.
    Problem,
}

impl ListLineKind {
    const ALL: [Self; 6] = [
        Self::Domain,
        Self::Address,
        Self::Network,
        Self::Comment,
        Self::Blank,
        Self::Problem,
    ];
}

/// One line of a list file with its note ("overrides X line N…", a problem's reason) and the
/// number of routes it holds now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListLine {
    pub line: usize,
    pub text: String,
    pub kind: ListLineKind,
    pub note: Option<String>,
    pub routes: u64,
}

impl ListLine {
    pub fn write(&self, w: &mut Writer) {
        w.usize(self.line);
        w.str(&self.text);
        w.choice(&self.kind, &ListLineKind::ALL);
        w.opt_str(self.note.as_ref());
        w.u64(self.routes);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            line: r.usize()?,
            text: r.string()?,
            kind: r.choice(&ListLineKind::ALL)?,
            note: r.opt_string()?,
            routes: r.u64()?,
        })
    }
}

/// One profile's `routes.txt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListFile {
    pub name: String,
    pub lines: Vec<ListLine>,
}

impl ListFile {
    fn write(&self, w: &mut Writer) {
        w.str(&self.name);
        w.list(&self.lines, ListLine::write);
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            name: r.string()?,
            lines: r.list(ListLine::read)?,
        })
    }
}

/// Every list: the answer to `GetAllLists` and to a list edit or save that went through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListsView {
    /// The `base_rev` the next edit sends.
    pub rev: u64,
    /// The combined text.
    pub text: String,
    /// In the user's order.
    pub files: Vec<ListFile>,
    pub problems: Vec<TextProblem>,
}

impl ListsView {
    pub fn write(&self, w: &mut Writer) {
        w.u64(self.rev);
        w.str(&self.text);
        w.list(&self.files, ListFile::write);
        w.list(&self.problems, DomainWire::text_problem);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            rev: r.u64()?,
            text: r.string()?,
            files: r.list(ListFile::read)?,
            problems: r.list(DomainWire::read_text_problem)?,
        })
    }
}

/// A line to remove, with its text, so a line that changed on disk is not removed by number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineRef {
    pub line: usize,
    pub text: String,
}

impl LineRef {
    pub fn write(&self, w: &mut Writer) {
        w.usize(self.line);
        w.str(&self.text);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            line: r.usize()?,
            text: r.string()?,
        })
    }
}
