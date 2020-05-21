mod patch;
mod repo;
mod tree;

use std::path::Path;

pub use repo::{GitCommit, GitRepo};
pub use tree::Tree;

pub fn run_stat<P,F,S>(
    path: P,
    verbose: bool,
    commit_filter: F,
    mut stat: S,
) 
where 
    P: AsRef<Path>,
    F: Fn(&GitCommit) -> bool,
    S: FnMut(&GitRepo, &GitCommit, &Tree, usize, usize),
{
    let repo = GitRepo::open(path).unwrap();

    let commit = repo.find_commit(repo::VersionSpec::Head).unwrap();

    if verbose { 
        eprintln!("Sorting commits (head = {})", commit.id().unwrap_or(git2::Oid::zero()));
    }

    let result = commit.topological_sort(&commit_filter).unwrap();
    
    if verbose { 
        eprintln!("Found {} commits to process", result.len());
    }

    let plan = result.plan();

    let mut trees = std::collections::BTreeMap::new();
    let mut base_line_tree = None;

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
                let patch = commit.diff_with(parent_commits.iter(), verbose).unwrap();
                if verbose {
                    eprintln!("Analyzing initial commit {}", commit.id().unwrap_or(git2::Oid::zero()));
                }
                tree::Tree::analyze_patch(&[&empty], patch.as_ref(), commit.author_id())
            } else {
                if verbose {
                    eprintln!("Analyzing boundary commit {}", commit.id().unwrap_or(git2::Oid::zero()));
                }
                if base_line_tree.is_none() {
                    let tree = tree::Tree::from_commit(&commit, repo.query_author_id("Older Code"), verbose);
                    base_line_tree = Some((commit.clone(), tree.clone()));
                    tree
                } else {
                    let (bc, bt)  = base_line_tree.as_ref().unwrap();
                    let patch = commit.diff_with([bc.clone()].iter(), verbose).unwrap();
                    tree::Tree::analyze_patch(&[bt], patch.as_ref(), repo.query_author_id("Older Code"))
                }
            }
        } else {
            let parent_commits = result.get_parent_commits(step.processing);
            let patch = commit.diff_with(parent_commits.iter(),verbose).unwrap();
            if verbose {
                eprintln!("Analyzing commit {} (merge from {} parents)", result.get_commit(step.processing).unwrap().id().unwrap_or(git2::Oid::zero()), parent_commits.len());
            }
            tree::Tree::analyze_patch(parents.as_ref(), patch.as_ref(), commit.author_id())
        };

        stat(&repo, &commit, &tree, i, plan.len());

        trees.insert(step.processing, tree);

        for remove_idx in step.expired.iter() {
            trees.remove(remove_idx);
        }
    }
}
