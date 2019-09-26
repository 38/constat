use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::author::AuthorStat;
use crate::file::RepoFile;

pub struct GitRepo(HashMap<PathBuf, RepoFile>);

impl GitRepo {
    pub fn empty() -> Self {
        GitRepo(HashMap::new())
    }
    pub fn apply_file_diff(
        &mut self,
        author: u16,
        old_file: Option<&Path>,
        new_file: Option<&Path>,
        patch: impl Iterator<Item = (Option<usize>, Option<usize>)>,
        stat: &mut AuthorStat,
    ) {
        if old_file.map_or(false, |f| !self.0.contains_key(f)) {
            self.0
                .insert(new_file.unwrap().to_path_buf(), RepoFile::empty());
        }

        if new_file.is_none() {
            let file = self.0.remove(old_file.unwrap()).unwrap();
            for author in file.iter() {
                stat.incrment_author(*author, -1);
            }
            return;
        }

        let after_patched = self.0[new_file.clone().unwrap()].update(author, patch, stat);

        *self
            .0
            .entry(new_file.unwrap().to_path_buf())
            .or_insert(RepoFile::empty()) = after_patched;
    }
}
