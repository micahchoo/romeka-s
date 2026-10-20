//! What the derive reads from disk, read in one place.
//!
//! The plan's spreadsheet is a source, not a media file, so it is not in the
//! file list the pattern matches. A spreadsheet that cannot be read is kept as
//! an error, so the preview can say so beside everything else.

use super::archive::{ArchiveError, ArchiveFile, ArchiveFolder};
use super::plan::ImportPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inputs {
    /// The folder's files, sorted, without the plan's spreadsheet.
    pub files: Vec<ArchiveFile>,
    /// Every `.csv` file in the folder, by relative path.
    pub csv_files: Vec<String>,
    /// The plan's spreadsheet bytes, or why they cannot be read.
    pub csv: Option<Result<Vec<u8>, String>>,
}

impl Inputs {
    /// Walk the import folder and read the plan's spreadsheet.
    ///
    /// # Errors
    /// [`ArchiveError::Io`] when the folder cannot be walked.
    pub fn load(folder: &ArchiveFolder, plan: &ImportPlan) -> Result<Self, ArchiveError> {
        let all = folder.files()?;
        let csv = plan.csv.as_ref().map(|csv| {
            folder
                .file(&csv.file)
                .and_then(|path| std::fs::read(path).map_err(ArchiveError::from))
                .map_err(|error| error.to_string())
        });
        Ok(Self::from_files(all, plan, csv))
    }

    /// Inputs from a file list already walked, for callers that hold one.
    #[must_use]
    pub fn from_files(
        all: Vec<ArchiveFile>,
        plan: &ImportPlan,
        csv: Option<Result<Vec<u8>, String>>,
    ) -> Self {
        let csv_files = all
            .iter()
            .filter(|file| file.path.to_ascii_lowercase().ends_with(".csv"))
            .map(|file| file.path.clone())
            .collect();
        let source = plan.csv.as_ref().map(|csv| csv.file.as_str());
        let files = all
            .into_iter()
            .filter(|file| Some(file.path.as_str()) != source)
            .collect();
        Self {
            files,
            csv_files,
            csv,
        }
    }
}
