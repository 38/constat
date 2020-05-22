mod analyzer;
mod options;
mod plotting;

use plotting::render_plot;
use options::ConstatOptions;
use std::collections::{BTreeMap, HashMap};

fn main() {
    let options = ConstatOptions::new();

    let mut author_info: HashMap<_, BTreeMap<_, usize>> = HashMap::new();

    let mut pb = None;

    let quiet = options.quiet || options.verbose;

    analyzer::run_stat(
        &options.repo_path,
        options.verbose,
        |commit| {
            let time = commit.get_timestamp();
            time.map_or(true, |ts| {
                options.since.map_or(true, |since| ts.date() >= since)
            })
        },
        |repo, commit, tree, _proc, total| {
            let date = commit.get_timestamp().unwrap().date();

            if !quiet {
                if pb.is_none() {
                    pb = Some(indicatif::ProgressBar::new(total as u64));
                }

                pb.as_ref().unwrap().inc(1);
            }

            for (author_id, count) in tree
                .stat(|f| options.patterns.iter().any(|p| p.matches_path(f)))
                .into_iter()
                .enumerate()
            {
                let cell = author_info
                    .entry(repo.query_author_name(author_id as u32).unwrap())
                    .or_insert_with(|| BTreeMap::new())
                    .entry(date)
                    .or_default();
                *cell = (*cell).max(count as usize);
            }
        },
    );

    render_plot(&mut author_info, &options);

    if options.open {
        open::that(options.out_path).ok();
    }
}
