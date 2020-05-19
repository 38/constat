mod analyzer;
mod options;
mod plotting;

use options::ConstatOptions;
use plotting::Renderer;

use plotters::prelude::*;

use std::collections::{HashMap, HashSet, BTreeMap};

fn main() {

    let options = ConstatOptions::new();

    let mut author_info: HashMap<_, BTreeMap<_, usize>> = HashMap::new();

    let mut pb = None;

    let quiet = options.quiet;

    analyzer::run_stat(&options.repo_path, |repo, commit, tree, _proc, total| {
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
    });

    let author_info = {
        let exclude_older = options.exclude_older;
        let mut max_loc: Vec<_> = author_info
            .iter()
            .filter_map(|(name, stat)| {
                Some((name.to_string(), *stat.iter().map(|x| x.1).max().unwrap()))
            })
            .collect();

        max_loc.sort_by_key(|x| std::cmp::Reverse(x.1));

        let mut to_keep = options.top;
        for idx in options.top..max_loc.len() {
            let (name, count) = &max_loc[idx];
            if options.pinned_author.iter().any(|p| p.matches(name)) {
                max_loc[to_keep] = (name.to_owned(), *count);
                to_keep += 1;
            }
        }

        max_loc.truncate(to_keep);

        let mut others = HashMap::new();

        let is_top_authors: HashSet<_> = max_loc.iter().map(|(name, _)| name.as_ref()).collect();

        for (name, stats) in author_info.iter() {
            if is_top_authors.contains(&name[..]) || (exclude_older && name == "Older Code") {
                continue;
            }

            for (t, c) in stats {
                *others.entry(t.clone()).or_insert(0) += c;
            }
        }

        let mut others: Vec<_> = others.into_iter().collect();
        others.sort();
        let mut buf = vec![];

        for (name, _) in max_loc {
            let stat = author_info.remove(&name).unwrap().into_iter().collect();
            buf.push((name.to_string(), stat));
        }

        if !others.is_empty() && !options.top_only {
            buf.push(("Others".to_string(), others));
        }

        buf.sort_by_key(|(_name, stats)| {
            stats.first().unwrap().0
        });

        buf
    };

    if options
        .out_path
        .extension()
        .map_or(true, |ext| ext == "svg")
    {
        let renderer = Renderer::new(
            options.repo_path,
            author_info,
            SVGBackend::new(&options.out_path, options.resolution),

        );

        renderer.draw();
    } else {
        let renderer = Renderer::new(
            options.repo_path,
            author_info,
            BitMapBackend::new(&options.out_path, options.resolution),
        );

        renderer.draw();
    }

    if options.open {
        open::that(options.out_path).ok();
    }
}
