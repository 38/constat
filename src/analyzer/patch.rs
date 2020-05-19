use git2::{DiffDelta, DiffLine};

use std::path::{Path, PathBuf};
/// Describe a line patch, either insersion or deletion
#[derive(Debug)]
pub enum LinePatch {
    Insert(u32),
    Delete(u32),
}

impl LinePatch {
    pub fn from_git2_object(diff: &DiffLine) -> Option<Self> {
        match diff.origin() {
            '+' => Some(LinePatch::Insert(diff.new_lineno().unwrap() - 1)),
            '-' => Some(LinePatch::Delete(diff.old_lineno().unwrap() - 1)),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn new_lineno(&self) -> Option<u32> {
        match self {
            Self::Insert(line) => Some(*line),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn old_lineno(&self) -> Option<u32> {
        match self {
            Self::Delete(line) => Some(*line),
            _ => None,
        }
    }
}

pub struct FilePatch {
    old_path: Option<PathBuf>,
    new_path: Option<PathBuf>,
    pub patch: Vec<LinePatch>,
}

impl FilePatch {
    pub fn from_git2_object(diff: &DiffDelta) -> Self {
        FilePatch {
            old_path: diff.old_file().path().map(|x| x.to_path_buf()),
            new_path: diff.new_file().path().map(|x| x.to_path_buf()),
            patch: vec![],
        }
    }

    pub fn old_path(&self) -> Option<&Path> {
        self.old_path.as_ref().map(AsRef::as_ref)
    }

    pub fn new_path(&self) -> Option<&Path> {
        self.new_path.as_ref().map(AsRef::as_ref)
    }

    pub fn push_line_diff(&mut self, diff: &DiffLine) {
        if let Some(diff) = LinePatch::from_git2_object(diff) {
            self.patch.push(diff);
        }
    }
}
pub struct TreePatch {
    pub new_author: u32,
    pub old_author: Option<u32>,
    files: Vec<FilePatch>,
}

impl TreePatch {
    pub fn empty(new_author: u32, old_author: Option<u32>) -> Self {
        TreePatch {
            files: vec![],
            new_author,
            old_author,
        }
    }

    pub fn push_file(&mut self, diff: &DiffDelta) {
        self.files.push(FilePatch::from_git2_object(diff));
    }

    pub fn files_mut(&mut self) -> &mut [FilePatch] {
        &mut self.files[..]
    }

    pub fn files(&self) -> &[FilePatch] {
        &self.files[..]
    }

    pub fn sort_patches(&mut self) {
        self.files
            .sort_by(|a, b| Ord::cmp(&a.new_path(), &b.new_path()))
    }
}
