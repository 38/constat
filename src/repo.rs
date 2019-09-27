use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::author::AuthorStat;
use crate::file::RepoFile;

use glob::Pattern;

pub struct GitRepo<'a> {
    tree: HashMap<PathBuf, RepoFile>,
    patterns: &'a [Pattern],
}

impl<'a> Drop for GitRepo<'a> {
    fn drop(&mut self) {
        for (_, file) in self.tree.iter_mut() {
            file.set_tracking_flag(false, None);
        }
    }
}

impl<'a> GitRepo<'a> {
    fn should_tracking<P: AsRef<Path>>(&self, path: P) -> bool {
        self.patterns
            .iter()
            .any(|pattern| pattern.matches_path(path.as_ref()))
    }
    pub fn empty(patterns: &'a [Pattern]) -> Self {
        GitRepo {
            tree: HashMap::new(),
            patterns,
        }
    }
    pub fn apply_file_diff(
        &mut self,
        author: u16,
        old_file: Option<&Path>,
        new_file: Option<&Path>,
        patch: impl Iterator<Item = (Option<usize>, Option<usize>)>,
        stat: &mut AuthorStat,
    ) {
        // First of all, let's make sure the old file exists
        // It's impossible that both old_file and new_file is None, at that time,
        // we are panicking anyway.
        let old_file = old_file.map_or(new_file.clone().unwrap(), |f| f);
        if !self.tree.contains_key(old_file) {
            let should_tracking = self.should_tracking(old_file);
            self.tree
                .insert(old_file.to_owned(), RepoFile::empty(should_tracking));
        }

        // Then if we are deleting the file, just remove it from the tree
        if new_file.is_none() {
            let mut file = self.tree.remove(old_file).unwrap();
            // Before we actually drop the file, make sure we are untracking this file
            file.set_tracking_flag(false, Some(stat));
            return;
        }

        let new_file = new_file.unwrap();

        if old_file == new_file {
            self.tree
                .entry(old_file.to_owned())
                .and_modify(|file| file.update(author, patch, stat));
        } else {
            let mut old_file = self.tree.remove(old_file).unwrap();

            if let Some(mut file) = self.tree.remove(new_file) {
                file.set_tracking_flag(false, Some(stat));
            }

            old_file.set_tracking_flag(self.should_tracking(new_file), Some(stat));
            old_file.update(author, patch, stat);

            self.tree.insert(new_file.to_owned(), old_file);
        }
    }
}
