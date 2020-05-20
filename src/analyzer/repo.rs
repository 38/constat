use git2::{Commit, Error, Oid, Repository};

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use super::patch::TreePatch;

use chrono::{DateTime, Duration, TimeZone, Utc};

#[derive(Default)]
struct AuthorCollection {
    name_id_map: HashMap<String, u32>,
    id_name_map: Vec<String>,
}

impl AuthorCollection {
    fn query_id(&mut self, name: &str) -> u32 {
        if let Some(&result) = self.name_id_map.get(name) {
            result
        } else {
            let ret = self.id_name_map.len() as u32;
            self.name_id_map.insert(name.to_string(), ret);
            self.id_name_map.push(name.to_string());
            ret
        }
    }
    #[allow(dead_code)]
    fn query_name(&self, id: u32) -> Option<&str> {
        self.id_name_map.get(id as usize).map(AsRef::as_ref)
    }
}

pub struct GitRepo {
    inner: Repository,
    authors: RefCell<AuthorCollection>,
}

impl GitRepo {
    pub fn query_author_id(&self, author: &str) -> u32 {
        self.authors.borrow_mut().query_id(author)
    }

    pub fn query_author_name(&self, id: u32) -> Option<String> {
        self.authors.borrow().query_name(id).map(|r| r.to_owned())
    }

    fn get_patch(
        &self,
        old_commit: Option<&Commit>,
        new_commit: &Commit,
    ) -> Result<TreePatch, Error> {
        let old_tree = old_commit.map(|c| c.tree().unwrap());
        let new_tree = new_commit.tree().unwrap();
        let old_aid =
            old_commit.map(|c| self.query_author_id(c.author().name().unwrap_or("<Unknown>")));
        let new_aid = self.query_author_id(new_commit.author().name().unwrap_or("<Unknown>"));
        let mut diff_option = git2::DiffOptions::new();
        diff_option.skip_binary_check(true);
        let mut diff = self.inner.diff_tree_to_tree(
            old_tree.as_ref(),
            Some(&new_tree),
            Some(&mut diff_option),
        )?;
        diff.find_similar(None)?;
        let ret = RefCell::new(TreePatch::empty(new_aid, old_aid));

        diff.foreach(
            &mut |file_diff, _| {
                let mut ret = ret.borrow_mut();
                ret.push_file(&file_diff);
                true
            },
            None,
            None,
            Some(&mut |_d, _h, l| {
                ret.borrow_mut()
                    .files_mut()
                    .last_mut()
                    .unwrap()
                    .push_line_diff(&l);
                true
            }),
        )?;

        ret.borrow_mut().sort_patches();

        Ok(ret.into_inner())
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let inner = Repository::open(path.as_ref())?;
        Ok(Self {
            inner,
            authors: RefCell::new(AuthorCollection::default()),
        })
    }

    pub fn find_commit<'a>(&self, version: VersionSpec<'a>) -> Result<GitCommit, Error> {
        let commit = match version {
            VersionSpec::Head => Some(self.inner.head()?.peel_to_commit()?),
            VersionSpec::Scratch => None,
            VersionSpec::Commit(id) => Some(self.inner.find_commit(Oid::from_str(id.as_ref())?)?),
        };

        Ok(GitCommit {
            repo: self,
            inner: commit,
        })
    }
}

#[allow(dead_code)]
pub enum VersionSpec<'a> {
    Head,
    Scratch,
    Commit(&'a str),
}

pub struct HistoryGraph<'a> {
    repo: &'a GitRepo,
    commits: Vec<Commit<'a>>,
    adj_table: Vec<Vec<usize>>,
    last_use: Vec<usize>,
}

#[derive(Debug)]
pub struct HistoryNode {
    pub processing: usize,
    pub expired: Vec<usize>,
}

impl<'a> HistoryGraph<'a> {
    pub fn len(&self) -> usize {
        self.commits.len()
    }

    pub fn get_commit(&self, idx: usize) -> Option<GitCommit<'a>> {
        self.commits.get(idx).map(|commit| GitCommit {
            repo: self.repo,
            inner: Some(commit.clone()),
        })
    }

    pub fn get_parent_idx(&self, idx: usize) -> Option<&[usize]> {
        self.adj_table.get(idx).map(AsRef::as_ref)
    }

    pub fn get_parent_commits<'b>(&'b self, idx: usize) -> Vec<GitCommit<'b>>
    where
        'a: 'b,
    {
        let mut ret = vec![];

        for &pid in self.get_parent_idx(idx).unwrap_or(&[][..]) {
            ret.push(GitCommit {
                repo: self.repo,
                inner: Some(self.commits[pid].clone()),
            });
        }
        ret
    }

    fn compute_last_use_array(&mut self) {
        self.last_use.resize(self.adj_table.len(), 0);
        for (idx, adj_node) in self.adj_table.iter().enumerate() {
            for &pid in adj_node.iter() {
                self.last_use[pid] = self.last_use[pid].max(idx);
            }
        }
        let len = self.len();
        self.last_use[len - 1] = len;
    }

    pub fn plan(&self) -> Vec<HistoryNode> {
        let mut expire_stack: Vec<_> = (0..self.len()).collect();
        expire_stack.sort_by_key(|&idx| self.len() - self.last_use[idx]);
        let mut ret = vec![];
        for processing in 0..self.len() {
            let mut expired = vec![];
            while let Some(&top) = expire_stack.last() {
                if self.last_use[top] <= processing {
                    expired.push(top);
                    expire_stack.pop();
                } else {
                    break;
                }
            }
            ret.push(HistoryNode {
                processing,
                expired,
            });
        }
        ret
    }
}

#[derive(Clone)]
pub struct GitCommit<'a> {
    repo: &'a GitRepo,
    inner: Option<Commit<'a>>,
}

impl<'a> GitCommit<'a> {
    pub fn scratch(&self) -> GitCommit {
        Self {
            repo: self.repo,
            inner: None,
        }
    }
    pub fn get_timestamp(&self) -> Option<DateTime<Utc>> {
        if let Some(commit) = &self.inner {
            let timestamp = commit.time().seconds();
            Some(Utc.ymd(1970, 1, 1).and_hms(0, 0, 0) + Duration::seconds(timestamp))
        } else {
            None
        }
    }

    pub fn get_author_by_name(&self, name: &str) -> u32 {
        self.repo.query_author_id(name)
    }

    pub fn author_id(&self) -> u32 {
        if let Some(git_obj) = self.inner.as_ref() {
            self.repo
                .query_author_id(git_obj.author().name().unwrap_or("<Unknown>"))
        } else {
            self.repo.query_author_id("<Unknown>")
        }
    }

    #[allow(dead_code)]
    pub fn author_name(&self) -> String {
        if let Some(git_obj) = self.inner.as_ref() {
            git_obj.author().name().unwrap_or("<Unknown>").to_string()
        } else {
            "<Unknown>".to_string()
        }
    }

    fn find_effctive_ancestors<'b>(commit: &Commit<'b>) -> Vec<Commit<'b>> {
        let mut ret = vec![];
        let mut queue = std::collections::VecDeque::new();

        queue.push_back(std::borrow::Cow::Borrowed(commit));

        let commit_time = Utc.ymd(1970, 1, 1) + Duration::seconds(commit.time().seconds());

        while let Some(cc) = queue.pop_front() {
            for parent in cc.parents() {
                let parent_commit_time =
                    Utc.ymd(1970, 1, 1) + Duration::seconds(parent.time().seconds());

                if commit_time == parent_commit_time
                    && commit.author().name() == parent.author().name()
                {
                    queue.push_back(Cow::Owned(parent));
                } else {
                    ret.push(parent);
                }
            }
        }

        ret
    }
    pub fn topological_sort<Pred: Fn(&GitCommit) -> bool + Clone>(
        self,
        predict: Pred,
    ) -> Result<HistoryGraph<'a>, Error> {
        if self.inner.is_none() {
            return Ok(HistoryGraph {
                repo: self.repo,
                adj_table: vec![],
                commits: vec![],
                last_use: vec![],
            });
        }
        let mut flag = HashMap::new();
        let mut adj_table = vec![];
        let mut ret = vec![];

        let mut stack = vec![self.inner.unwrap()];
        const INVALID_IDX: usize = !0usize;

        while let Some(root) = stack.pop() {
            let id = root.id();

            match flag.get(&id) {
                None => {
                    flag.insert(id, INVALID_IDX);
                    let should_recurse = predict(&GitCommit {
                        repo: self.repo,
                        inner: Some(root.clone()),
                    });
                    stack.push(root.clone());
                    if should_recurse {
                        for parent in Self::find_effctive_ancestors(&root) {
                            if !flag.contains_key(&parent.id()) {
                                stack.push(parent);
                            }
                        }
                    }
                }
                Some(&ofs) if ofs == INVALID_IDX => {
                    *flag.get_mut(&id).unwrap() = ret.len();
                    let mut adj_ids: Vec<_> = Self::find_effctive_ancestors(&root)
                        .into_iter()
                        .map(|p| p.id())
                        .collect();
                    adj_ids.sort();
                    let mut j = if adj_ids.is_empty() { 0 } else { 1 };

                    for i in 1..adj_ids.len() {
                        if adj_ids[j - 1] != adj_ids[i] {
                            j += 1;
                        }
                    }

                    let adj_node: Vec<_> = adj_ids[..j]
                        .iter()
                        .filter_map(|hash| flag.get(&hash).map(|x| *x))
                        .collect();

                    ret.push(root);
                    adj_table.push(adj_node);
                }
                Some(_) => {}
            }
        }

        let mut ret = HistoryGraph {
            repo: self.repo,
            adj_table,
            commits: ret,
            last_use: vec![],
        };
        ret.compute_last_use_array();
        Ok(ret)
    }

    pub fn is_initial_commit(&self) -> bool {
        if let Some(inner) = self.inner.as_ref() {
            if inner.parent_count() == 0 {
                return true;
            }
            Self::find_effctive_ancestors(inner).len() == 0
        } else {
            true
        }
    }

    pub fn diff_with<'b, BaseIter: IntoIterator<Item = &'b GitCommit<'b>>>(
        &self,
        base: BaseIter,
    ) -> Result<Vec<TreePatch>, Error> {
        let mut ret = vec![];
        if let Some(root) = self.inner.as_ref() {
            let mut empty = true;
            for commit in base.into_iter() {
                empty = false;
                let patch = self.repo.get_patch(commit.inner.as_ref(), &root)?;
                ret.push(patch);
            }
            if empty {
                let patch = self.repo.get_patch(None, &root)?;
                ret.push(patch);
            }
        }
        ret.iter_mut().for_each(|p| {
            p.files_mut()
                .sort_by_key(|k| k.new_path().map(ToOwned::to_owned))
        });
        Ok(ret)
    }
}
