use chrono::{Date, TimeZone, Utc};
use git2::{BranchType, Commit, Error as GitError, Oid, Repository};
use glob::Pattern;

use std::cell::{Ref, RefCell};
use std::path::{Path, PathBuf};

use crate::author::{AuthorIndex, AuthorStat};
use crate::repo::GitRepo;

#[allow(dead_code)]
pub enum VersionSpec {
    Head,
    Commit(Oid),
    Branch(String),
    FirstAfter(Date<Utc>),
    LastBefore(Date<Utc>),
    Scratch,
}

impl VersionSpec {
    fn get_commit<'a>(&self, repo: &'a Repository) -> Result<Option<Commit<'a>>, GitError> {
        Ok(match self {
            VersionSpec::Head => Some(repo.head()?.peel_to_commit()?),
            VersionSpec::Commit(id) => Some(repo.find_commit(*id)?),
            VersionSpec::Branch(ref name) => Some(
                repo.find_branch(name.as_ref(), BranchType::Local)?
                    .into_reference()
                    .peel_to_commit()?,
            ),
            VersionSpec::FirstAfter(date) => {
                let target_ts =
                    (date.and_hms(23, 59, 59) - Utc.ymd(1970, 1, 1).and_hms(0, 0, 0)).num_seconds();
                let mut commit = repo.head()?.peel_to_commit()?;

                if commit.time().seconds() <= target_ts {
                    return Ok(None);
                }

                while let Ok(next_commit) = commit.parent(0) {
                    if next_commit.time().seconds() <= target_ts {
                        return Ok(Some(commit));
                    }

                    commit = next_commit;
                }
                Some(commit)
            }
            VersionSpec::LastBefore(date) => {
                let target_ts =
                    (date.and_hms(23, 59, 59) - Utc.ymd(1970, 1, 1).and_hms(0, 0, 0)).num_seconds();
                let mut commit = repo.head()?.peel_to_commit()?;
                while commit.time().seconds() > target_ts {
                    if let Ok(next_commit) = commit.parent(0) {
                        commit = next_commit;
                    } else {
                        return Ok(None);
                    }
                }

                Some(commit)
            }
            VersionSpec::Scratch => None,
        })
    }
}

pub struct PendingStat<'a> {
    path: PathBuf,
    base_commit: VersionSpec,
    last_commit: VersionSpec,
    patterns: &'a [Pattern],
}

impl<'a> PendingStat<'a> {
    pub fn new<P: AsRef<Path>>(path: P, patterns: &'a [Pattern]) -> Self {
        Self {
            path: path.as_ref().to_owned(),
            last_commit: VersionSpec::Head,
            base_commit: VersionSpec::Scratch,
            patterns,
        }
    }

    #[allow(dead_code)]
    pub fn base_commit(&mut self, last: VersionSpec) -> &mut Self {
        self.base_commit = last;
        self
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
        eprintln!("Analyzing commits ...");
        let repo = Repository::open(self.path.as_path())?;

        let base_commit = self.base_commit.get_commit(&repo)?;

        let last_commit = self.last_commit.get_commit(&repo)?.unwrap();
        /*match self.last_commit {
            VersionSpec::Head => repo.head()?.peel_to_commit()?,
            VersionSpec::Commit(id) => repo.find_commit(id)?,
            VersionSpec::Branch(ref name) => repo
                .find_branch(name.as_ref(), BranchType::Local)?
                .into_reference()
                .peel_to_commit()?,
        };*/

        let mut commits = vec![last_commit];
        let mut has_nonempty_base = false;

        while let Ok(commit) = commits.last().unwrap().parent(0) {
            let should_exit = base_commit
                .as_ref()
                .map_or(false, |x| x.id() == commit.id());
            commits.push(commit);
            if should_exit {
                has_nonempty_base = true;
                break;
            }
        }

        let mut model = GitRepo::empty(self.patterns);

        let mut author_stat = AuthorStat::new();
        let mut author_index = RefCell::new(AuthorIndex::new());

        for (i, next) in (0..).zip(commits.iter().rev()) {
            let prev_tree = if i == 0 {
                None
            } else {
                commits[commits.len() - i].tree().ok()
            };

            let author = if i == 0 && has_nonempty_base {
                author_index.get_mut().get_id_by_name("Older Code")
            } else {
                author_index
                    .get_mut()
                    .get_id_by_name(next.author().name().unwrap_or("Unknown"))
            };

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
