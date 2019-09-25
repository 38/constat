use std::collections::HashMap;

use git2::{Commit, Repository};
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};

use plotters::prelude::*;

use std::env::args;

struct AuthorIndex {
    name2id: HashMap<String, u16>,
    id2name: Vec<String>,
}

impl AuthorIndex {
    fn new() -> Self {
        Self {
            name2id: HashMap::new(),
            id2name: vec![],
        }
    }

    fn get_id_by_name(&mut self, name: &str) -> u16 {
        if !self.name2id.contains_key(name) {
            let idx = self.id2name.len();
            self.id2name.push(name.to_string());
            self.name2id.insert(name.to_string(), idx as u16);
        }
        return self.name2id[name] as u16;
    }

    fn get_name_by_id(&self, idx: u16) -> Option<&str> {
        if idx as usize >= self.id2name.len() {
            return None;
        }
        Some(self.id2name[idx as usize].as_ref())
    }
}

struct AuthorStat(Vec<usize>);

impl AuthorStat {
    fn print(&self, a: &AuthorIndex) {
        for (i, c) in (0..).zip(self.0.iter()) {
            println!("{} {}", a.get_name_by_id(i).unwrap(), c);
        }
    }
    fn incrment_author(&mut self, author: u16, delta: i32) {
        for _ in self.0.len()..=author as usize {
            self.0.push(0);
        }

        if delta > 0 {
            self.0[author as usize] += 1;
        } else {
            self.0[author as usize] -= 1;
        }
    }
}

struct RepoFile(Vec<u16>);

impl RepoFile {
    fn update(
        &self,
        author: u16,
        mut patch: impl Iterator<Item = (Option<usize>, Option<usize>)>,
        stat: &mut AuthorStat,
    ) -> Self {
        let mut ret = RepoFile(vec![]);

        let mut next_patch = patch.next();
        let mut new_pos = 0;
        let mut old_pos = 0;

        while next_patch.is_some() || old_pos < self.0.len() {
            let patch_applied = match &next_patch {
                Some((Some(old), Some(_new))) if *old == old_pos => {
                    ret.0.push(author);
                    stat.incrment_author(self.0[old_pos], -1);
                    stat.incrment_author(author, 1);
                    new_pos += 1;
                    old_pos += 1;
                    true
                }
                Some((None, Some(new))) if *new == new_pos => {
                    ret.0.push(author);
                    new_pos += 1;
                    stat.incrment_author(author, 1);
                    true
                }
                Some((Some(old), None)) if *old == old_pos => {
                    stat.incrment_author(self.0[old_pos], -1);
                    old_pos += 1;
                    true
                }
                Some((None, None)) => true,
                _ => {
                    if self.0.len() > old_pos {
                        ret.0.push(self.0[old_pos]);
                    }
                    new_pos += 1;
                    old_pos += 1;
                    false
                }
            };

            if patch_applied {
                next_patch = patch.next();
            }
        }

        ret
    }
}

struct GitRepo(HashMap<PathBuf, RepoFile>);

impl GitRepo {
    fn apply_file_diff(
        &mut self,
        author: u16,
        old_file: Option<&Path>,
        new_file: Option<&Path>,
        patch: impl Iterator<Item = (Option<usize>, Option<usize>)>,
        stat: &mut AuthorStat,
    ) {
        if old_file.map_or(false, |f| !self.0.contains_key(f)) {
            self.0
                .insert(new_file.unwrap().to_path_buf(), RepoFile(vec![]));
        }

        if new_file.is_none() {
            let file = self.0.remove(old_file.unwrap()).unwrap();
            for author in file.0 {
                stat.incrment_author(author, -1);
            }
            return;
        }

        let after_patched = self.0[new_file.clone().unwrap()].update(author, patch, stat);

        *self
            .0
            .entry(new_file.unwrap().to_path_buf())
            .or_insert(RepoFile(vec![])) = after_patched;
    }
}

fn main() {
    let repo_path = args().skip(1).next().unwrap();

    let mut authors = AuthorIndex::new();

    let repo = Repository::open(&repo_path).unwrap();

    let mut head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    let mut commits = vec![head_commit];
    while let Ok(commit) = commits.last().unwrap().parent(0) {
        commits.push(commit);
    }

    println!("Found {} commits", commits.len());

    let mut repo_model = GitRepo(HashMap::new());
    let mut stat = AuthorStat(vec![]);

    let mut data = vec![];

    for (i, next) in (0..).zip(commits.iter().rev()) {
        if i % 100 == 0 {
            println!("{} commits has been scanned", i);
        }
        let prev_tree = if i == 0 {
            None
        } else {
            Some(commits[commits.len() - i].tree().unwrap())
        };
        let author = authors.get_id_by_name(next.author().name().unwrap());
        let diff = repo
            .diff_tree_to_tree(prev_tree.as_ref(), Some(&next.tree().unwrap()), None)
            .unwrap();

        let mut patches = std::cell::RefCell::new(vec![]);

        // TODO handle bin diff
        diff.foreach(
            &mut |a, _| {
                let before_file = a.old_file().path().map(|x| x.to_path_buf());
                let after_file = a.new_file().path().map(|x| x.to_path_buf());
                patches.borrow_mut().push((before_file, after_file, vec![]));
                true
            },
            None,
            None,
            Some(&mut |d, _h, l| {
                if l.origin() == '+' || l.origin() == '-' {
                    patches.borrow_mut().last_mut().unwrap().2.push((
                        l.old_lineno().map(|x| x as usize - 1),
                        l.new_lineno().map(|x| x as usize - 1),
                    ));
                }

                true
            }),
        );

        for (before, after, patch) in patches.into_inner() {
            let before_path = before.as_ref().map(|x| x.as_path());
            let after_path = after.as_ref().map(|x| x.as_path());
            repo_model.apply_file_diff(
                author,
                before_path,
                after_path,
                patch.into_iter(),
                &mut stat,
            );

        }
        println!("{} {}", next.id(), repo_model.0.get("hphp/test/zend/bad/zend/cast_to_string.php.exp".parse::<PathBuf>().unwrap().as_path()).unwrap_or(&RepoFile(vec![])).0.len());

        let ts = chrono::Utc.ymd(1970, 1, 1) + chrono::Duration::seconds(next.time().seconds());

        data.push((ts, stat.0.clone()));
    }

    let mut significant_authors = vec![false; authors.id2name.len()];

    for (t, cnt) in data.iter() {
        for (i, n) in (0..).zip(cnt.iter()) {
            if *n > 100 {
                significant_authors[i] = true;
            }
        }
    }

    for (i, s) in (0..).zip(significant_authors.iter()) {
        if *s {
            println!("{:?}", authors.get_name_by_id(i));
        }
    }

    let mut max_loc = 0;

    let mut stack: Vec<(u16, Vec<i32>)> = vec![];

    for (i, s) in (0..).zip(significant_authors.iter()) {
        if *s {
            stack.push((
                i as u16,
                data.iter()
                    .map(|(t, c)| *c.get(i).unwrap_or(&0) as i32)
                    .collect(),
            ));

            max_loc = max_loc.max(
                *stack[stack.len() - 1]
                    .1
                    .iter()
                    .max()
                    .unwrap_or(&max_loc.clone()),
            );
        }
    }

    println!("{}", max_loc);

    let root = BitMapBackend::new("stat.png", (1024, 768)).into_drawing_area();

    root.fill(&WHITE).unwrap();

    let mut chart = ChartBuilder::on(&root)
        .set_label_area_size(LabelAreaPosition::Left, 60)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .caption(format!("{} contributors stat", repo_path), ("Arial", 40))
        .build_ranged(
            data.iter().map(|(t, _)| t.clone()).min().unwrap()
                ..data.iter().map(|(t, _)| t.clone()).max().unwrap(),
            0..max_loc,
        )
        .unwrap();

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_mesh()
        .draw()
        .unwrap();

    /*for i in 0..data.len() {
        for j in 1..stack.len() {
            stack[j].1[i] += stack[j-1].1[i];
        }
    }*/

    for j in 0..stack.len() {
        let c = Palette99::pick(j);
        chart
            .draw_series(LineSeries::new(
                data.iter()
                    .zip(stack[j].1.iter())
                    .map(|((t, _), c)| (t.clone(), *c)),
                &c,
            ))
            .unwrap()
            .label(authors.get_name_by_id(stack[j].0).unwrap())
            .legend(move |(x, y)| plotters::prelude::Path::new(vec![(x, y), (x + 20, y)], &c));
    }

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::MiddleLeft)
        .border_style(&BLACK)
        .draw()
        .unwrap();
}
