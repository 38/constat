mod patch;
mod repo;
mod tree;

use std::path::Path;

pub use repo::{GitCommit, GitRepo};
pub use tree::Tree;

pub fn run_stat<P: AsRef<Path>, S: FnMut(&GitRepo, &GitCommit, &Tree, usize, usize)>(
    path: P,
    mut stat: S,
) {
    let repo = GitRepo::open(path).unwrap();

    let commit = repo.find_commit(repo::VersionSpec::Head).unwrap();

    let result = commit.topological_sort(|_| true).unwrap();

    let plan = result.plan();

    let mut trees = std::collections::BTreeMap::new();

    for i in 0..plan.len() {
        let step = &plan[i];

        let commit = result.get_commit(step.processing).unwrap();

        let mut patch = commit.diff_with(commit.parents().iter()).unwrap();
        let mut parents: Vec<_> = result
            .get_parent_idx(step.processing)
            .unwrap()
            .iter()
            .map(|pid| &trees[pid])
            .collect();

        let tree = if parents.len() == 0 {
            let empty = tree::Tree::empty();
            tree::Tree::analyze_patch(&[&empty], patch.as_ref(), commit.author_id())
        } else {
            tree::Tree::analyze_patch(parents.as_ref(), patch.as_ref(), commit.author_id())
        };

        stat(&repo, &commit, &tree, i, plan.len());

        trees.insert(step.processing, tree);

        for remove_idx in step.expired.iter() {
            trees.remove(remove_idx);
        }
    }
}
