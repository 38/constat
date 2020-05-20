mod patch;
mod repo;
mod tree;

use std::path::Path;

pub use repo::{GitCommit, GitRepo};
pub use tree::Tree;

pub fn run_stat<
    P: AsRef<Path>,
    F: Fn(&GitCommit) -> bool,
    S: FnMut(&GitRepo, &GitCommit, &Tree, usize, usize),
>(
    path: P,
    commit_filter: F,
    mut stat: S,
) {
    let repo = GitRepo::open(path).unwrap();

    let commit = repo.find_commit(repo::VersionSpec::Head).unwrap();

    /*{
        let test_commit = repo.find_commit(repo::VersionSpec::Commit("9fdb62af92c741addbea15545f214a6e89460865")).unwrap();
        let pc = test_commit.parents();
        let ts:Vec<_> = pc.iter().map(|_| Tree::empty()).collect();
        let tsr:Vec<_> = ts.iter().collect();
        let diff = test_commit.diff_with(pc.iter()).unwrap();
        Tree::analyze_patch(&tsr, &diff, 0);
    }*/

    let result = commit.topological_sort(&commit_filter).unwrap();

    let plan = result.plan();

    let mut trees = std::collections::BTreeMap::new();

    for i in 0..plan.len() {
        let step = &plan[i];

        let commit = result.get_commit(step.processing).unwrap();

        let parents: Vec<_> = result
            .get_parent_idx(step.processing)
            .unwrap()
            .iter()
            .map(|pid| &trees[pid])
            .collect();

        let tree = if parents.len() == 0 {
            if commit.is_initial_commit() {
                let empty = tree::Tree::empty();
                let parent_commits = result.get_parent_commits(step.processing);
                let patch = commit.diff_with(parent_commits.iter()).unwrap();
                tree::Tree::analyze_patch(&[&empty], patch.as_ref(), commit.author_id())
            } else {
                tree::Tree::from_commit(&commit, repo.query_author_id("Older Code"))
            }
        } else {
            let parent_commits = result.get_parent_commits(step.processing);
            let patch = commit.diff_with(parent_commits.iter()).unwrap();
            tree::Tree::analyze_patch(parents.as_ref(), patch.as_ref(), commit.author_id())
        };

        stat(&repo, &commit, &tree, i, plan.len());

        trees.insert(step.processing, tree);

        for remove_idx in step.expired.iter() {
            trees.remove(remove_idx);
        }
    }
}
