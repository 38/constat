use git2::{BranchType, Commit, Error as GitError, Oid, Repository};

use std::cell::{Ref, RefCell};
use std::path::{Path, PathBuf};

use crate::author::{AuthorIndex, AuthorStat};
use crate::repo::GitRepo;

#[allow(dead_code)]
pub enum VersionSpec {
    Head,
    Commit(Oid),
    Branch(String),
}

pub struct PendingStat {
    path: PathBuf,
    last_commit: VersionSpec,
}

impl PendingStat {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_owned(),
            last_commit: VersionSpec::Head,
        }
    }

    #[allow(dead_code)]
    pub fn last_commit(&mut self, last: VersionSpec) -> &mut Self {
        self.last_commit = last;
        self
    }

    pub fn run(
        self,
        mut commit_scanned: impl FnMut(&Commit, &[usize], Ref<AuthorIndex>, usize),
    ) -> Result<(), GitError> {
        let repo = Repository::open(self.path.as_path())?;

        let last_commit = match self.last_commit {
            VersionSpec::Head => repo.head()?.peel_to_commit()?,
            VersionSpec::Commit(id) => repo.find_commit(id)?,
            VersionSpec::Branch(ref name) => repo
                .find_branch(name.as_ref(), BranchType::Local)?
                .into_reference()
                .peel_to_commit()?,
        };

        let mut commits = vec![last_commit];

        while let Ok(commit) = commits.last().unwrap().parent(0) {
            commits.push(commit);
        }

        let mut model = GitRepo::empty();
        let mut author_stat = AuthorStat::new();
        let mut author_index = RefCell::new(AuthorIndex::new());
        for (i, next) in (0..).zip(commits.iter().rev()) {
            let prev_tree = if i == 0 {
                None
            } else {
                commits[commits.len() - i].tree().ok()
            };

            let author = author_index
                .get_mut()
                .get_id_by_name(next.author().name().unwrap_or("Unknown"));

            let cur_tree = next.tree().ok();

            let diff = repo.diff_tree_to_tree(prev_tree.as_ref(), cur_tree.as_ref(), None)?;

            let patches = RefCell::new(vec![]);

            diff.foreach(
                &mut |a, _| {
                    let before_file = a.old_file().path().map(|x| x.to_path_buf());
                    let after_file = a.new_file().path().map(|x| x.to_path_buf());
                    patches.borrow_mut().push((before_file, after_file, vec![]));
                    true
                },
                None,
                None,
                Some(&mut |_d, _h, l| {
                    if l.origin() == '+' || l.origin() == '-' {
                        patches.borrow_mut().last_mut().unwrap().2.push((
                            l.old_lineno().map(|x| x as usize - 1),
                            l.new_lineno().map(|x| x as usize - 1),
                        ));
                    }
                    true
                }),
            )?;

            for (before, after, patch) in patches.into_inner() {
                let before_path = before.as_ref().map(|x| x.as_path());
                let after_path = after.as_ref().map(|x| x.as_path());
                model.apply_file_diff(
                    author,
                    before_path,
                    after_path,
                    patch.into_iter(),
                    &mut author_stat,
                );
            }

            commit_scanned(
                &next,
                author_stat.stats(),
                author_index.borrow(),
                commits.len(),
            );
        }

        Ok(())
    }
}
