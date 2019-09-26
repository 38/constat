mod author;
mod file;
mod plotting;
mod repo;
mod stat;

use plotting::Renderer;
use stat::PendingStat;

use chrono::{Duration, TimeZone, Utc};
use clap::{load_yaml, App};
use plotters::prelude::*;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn main() {
    let option_spec = load_yaml!("cli.yml");
    let options = App::from_yaml(option_spec).get_matches();

    let repo_path: PathBuf =
        std::fs::canonicalize(options.value_of("repository").unwrap_or(".")).unwrap();

    let top: usize = options.value_of("top").unwrap_or("5").parse().unwrap();

    let out: PathBuf = options
        .value_of("output")
        .map(|x| x.to_string())
        .unwrap_or_else(|| {
            format!(
                "{}.constat.png",
                repo_path
                    .file_name()
                    .unwrap_or_else(|| "unknown-repo".as_ref())
                    .to_string_lossy()
                    .to_owned()
            )
        })
        .into();

    let resolution: (u32, u32) = {
        let mut parser = options
            .value_of("resolution")
            .unwrap_or("1024x768")
            .split(|x| x == 'x')
            .take(2)
            .map(|s| s.parse().unwrap());
        (parser.next().unwrap(), parser.next().unwrap())
    };

    let ps = PendingStat::new(&repo_path);

    let mut author_info: HashMap<_, Vec<(_, usize)>> = HashMap::new();

    let mut pb = None;

    ps.run(|commit, stat, authors, total| {
        if pb.is_none() {
            pb = Some(indicatif::ProgressBar::new(total as u64));
        }

        pb.as_ref().unwrap().inc(1);
        for (i, s) in (0..).zip(stat.iter()) {
            let value = author_info
                .entry(authors.get_name_by_id(i).unwrap_or("N/A").to_string())
                .or_insert_with(|| vec![]);

            let timestamp = Utc.ymd(1970, 1, 1) + Duration::seconds(commit.time().seconds());
            if let Some(what) = value.last_mut() {
                if what.0 == timestamp {
                    what.1 = what.1.max(*s);
                    continue;
                }
            }

            value.push((timestamp, *s));
        }
    })
    .unwrap();

    let author_info = {
        let mut max_loc: Vec<_> = author_info
            .iter()
            .map(|(name, stat)| (name.to_string(), stat.iter().map(|x| x.1).max().unwrap()))
            .collect();

        max_loc.sort_by_key(|x| std::cmp::Reverse(x.1));

        max_loc.truncate(top);

        let mut others = HashMap::new();

        let mut is_top_authors = HashSet::new();

        for (name, _) in &max_loc {
            is_top_authors.insert(&name[..]);
        }

        for (name, stats) in author_info.iter() {
            if is_top_authors.contains(&name[..]) {
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
            buf.push((name.to_string(), author_info.remove(&name).unwrap()));
        }

        if !others.is_empty() {
            buf.push(("others".to_string(), others));
        }

        buf.sort_by_key(|(_, stats)| stats.first().unwrap().0);

        buf
    };

    if out.extension().map_or(true, |ext| ext == "svg") {
        let renderer = Renderer::new(repo_path, author_info, SVGBackend::new(&out, resolution));

        renderer.draw();
    } else {
        let renderer = Renderer::new(repo_path, author_info, BitMapBackend::new(&out, resolution));

        renderer.draw();
    }
}
