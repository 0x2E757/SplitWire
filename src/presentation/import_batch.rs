//! The files of one import: each gets a `PreviewImport` for its
//! current name, a name typed as the user likes, a colour; Import sends one `Import` per file, in
//! order, and stops at the first failure.

use std::sync::Arc;

use super::PickedFile;
use crate::ipc::{ConfText, ProfileSummary};
use crate::profile::{CheckLevel, ImportPreview, Pin, ProfileName};

pub struct ReviewFile {
    pub file_name: String,
    pub text: Arc<ConfText>,
    pub name: String,
    pub colour: Option<Pin>,
    /// The latest preview and the name it was made for.
    pub preview: Option<ImportPreview>,
    pub previewed: Option<String>,
    pub imported: bool,
}

impl ReviewFile {
    /// The name a file proposes: its stem.
    pub fn stem(file_name: &str) -> String {
        let stem = file_name.rsplit(['\\', '/']).next().unwrap_or(file_name);
        stem.strip_suffix(".conf")
            .or_else(|| stem.strip_suffix(".CONF"))
            .unwrap_or(stem)
            .to_owned()
    }

    /// The preview of the current name, once it arrived.
    pub fn current(&self) -> Option<&ImportPreview> {
        self.preview
            .as_ref()
            .filter(|_| self.previewed.as_deref() == Some(self.name.as_str()))
    }

    pub fn replace(&self) -> bool {
        self.current().is_some_and(|p| p.replace)
    }

    pub fn colour(&self) -> Pin {
        self.colour
            .or_else(|| self.preview.as_ref().map(|p| p.colour))
            .unwrap_or_else(|| Pin::new(1).expect("pin 1"))
    }
}

/// A file that could not be read, with the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnreadFile {
    pub file_name: String,
    pub reason: String,
}

pub struct ImportBatch {
    pub files: Vec<ReviewFile>,
    pub unread: Vec<UnreadFile>,
    /// The profile a Replace .conf… is for.
    pub replace: Option<String>,
    /// The index of the file being imported.
    pub importing: Option<usize>,
    pub failure: Option<String>,
    pub done: Vec<ProfileSummary>,
}

impl ImportBatch {
    pub fn new(picked: Vec<PickedFile>, replace: Option<String>) -> Self {
        let mut files = Vec::new();
        let mut unread = Vec::new();
        for file in picked {
            match file.text {
                Ok(text) => {
                    let name = match (&replace, files.is_empty()) {
                        (Some(target), true) => target.clone(),
                        _ => ReviewFile::stem(&file.file_name),
                    };
                    files.push(ReviewFile {
                        file_name: file.file_name,
                        text,
                        name,
                        colour: None,
                        preview: None,
                        previewed: None,
                        imported: false,
                    });
                }
                Err(reason) => unread.push(UnreadFile {
                    file_name: file.file_name,
                    reason,
                }),
            }
        }
        // A Replace .conf… takes one file.
        if replace.is_some() {
            files.truncate(1);
        }
        Self {
            files,
            unread,
            replace,
            importing: None,
            failure: None,
            done: Vec::new(),
        }
    }

    /// The window's own name rules on top of the preview's: a name is needed, and two files of
    /// one batch cannot share one.
    pub fn name_problem(&self, index: usize) -> Option<String> {
        let file = &self.files[index];
        if let Err(reason) = ProfileName::parse(&file.name) {
            return Some(reason);
        }
        if self
            .files
            .iter()
            .enumerate()
            .any(|(i, other)| i != index && other.name.eq_ignore_ascii_case(&file.name))
        {
            return Some("Two files would get the same name".into());
        }
        file.current().and_then(|p| p.name_problem.clone())
    }

    /// Why a file cannot be imported now; `None` when it can.
    pub fn blocked(&self, index: usize) -> Option<String> {
        if let Some(problem) = self.name_problem(index) {
            return Some(problem);
        }
        let preview = self.files[index].current()?;
        preview.problems().into_iter().next().or_else(|| {
            preview
                .checks
                .iter()
                .find(|c| c.level == CheckLevel::Bad)
                .map(|c| c.title.clone())
        })
    }

    /// Import is possible: every file has its preview and nothing blocks.
    pub fn ready(&self) -> bool {
        !self.files.is_empty()
            && self.importing.is_none()
            && (0..self.files.len())
                .all(|i| self.files[i].current().is_some() && self.blocked(i).is_none())
    }

    /// Files whose preview is missing or stale: `(index, name)`.
    pub fn to_preview(&self) -> Vec<(usize, String)> {
        self.files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.previewed.as_deref() != Some(f.name.as_str()))
            .filter(|(_, f)| ProfileName::parse(&f.name).is_ok())
            .map(|(i, f)| (i, f.name.clone()))
            .collect()
    }

    /// The next file to import, after `importing`.
    pub fn next(&self) -> Option<usize> {
        self.files.iter().position(|f| !f.imported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picked(name: &str) -> PickedFile {
        PickedFile {
            file_name: name.into(),
            text: Ok(Arc::new(ConfText::new("[Interface]\n".into()))),
        }
    }

    #[test]
    fn names_come_from_the_files_and_are_checked_together() {
        let batch = ImportBatch::new(
            vec![
                picked("C:\\conf\\Partner.conf"),
                picked("partner.conf"),
                picked("Acme Corp (old).conf"),
                PickedFile {
                    file_name: "big.conf".into(),
                    text: Err("larger than 64 KB".into()),
                },
            ],
            None,
        );
        assert_eq!(batch.files[0].name, "Partner");
        assert_eq!(batch.unread[0].reason, "larger than 64 KB");
        assert_eq!(
            batch.name_problem(0).unwrap(),
            "Two files would get the same name"
        );
        assert_eq!(
            batch.name_problem(2).unwrap(),
            "Letters, digits, . _ - only (no spaces or brackets)"
        );
        assert_eq!(batch.to_preview().len(), 2);
        assert!(!batch.ready());
    }

    #[test]
    fn a_replace_takes_the_target_name_and_one_file() {
        let batch = ImportBatch::new(
            vec![picked("new.conf"), picked("b.conf")],
            Some("AcmeCorp".into()),
        );
        assert_eq!(batch.files.len(), 1);
        assert_eq!(batch.files[0].name, "AcmeCorp");
    }
}
