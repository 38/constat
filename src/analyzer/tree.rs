use super::patch::{FilePatch, TreePatch};
use super::GitCommit;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct LineBlock {
    author_id: u32,
    size: u32,
}

#[derive(Clone)]
pub struct Tree<'a> {
    root: HashMap<PathBuf, Cow<'a, Vec<LineBlock>>>,
}

fn get_patch_filename_list(patch: &[TreePatch]) -> Vec<(Option<&Path>, Option<&Path>)> {
    let mut file_name_vec: Vec<_> = patch
        .iter()
        .map(|tree| {
            tree.files()
                .iter()
                .map(|file| (file.old_path(), file.new_path()))
        })
        .flatten()
        .collect();
    file_name_vec.sort_by(|a, b| Ord::cmp(&a.1, &b.1));
    let mut j = 1;
    for i in 1..file_name_vec.len() {
        if file_name_vec[j - 1] == file_name_vec[i] {
            continue;
        } else {
            file_name_vec[j] = file_name_vec[i];
            j += 1;
        }
    }
    file_name_vec.resize(j, (None, None));
    file_name_vec
}

fn get_related_authors(patches: &[TreePatch]) -> Vec<u32> {
    patches
        .iter()
        .map(|patch| patch.old_author.unwrap_or(patch.new_author))
        .collect()
}

#[derive(Debug)]
struct Addition {
    author: u32,
    line: u32,
}

fn merge_file_patch<'a>(
    patches: impl Iterator<Item = (u32, Option<&'a FilePatch>)>,
    merger: u32,
) -> Vec<Addition> {
    let mut patches: Vec<_> = patches
        .map(|(author, patch)| {
            if let Some(fp) = patch {
                let line_patch = &fp.patch[..];
                (author, line_patch)
            } else {
                (author, &[][..])
            }
        })
        .collect();

    let mut ret = vec![];

    let sum = (1 + patches.len()) * patches.len() / 2;

    loop {
        for (_, p) in patches.iter_mut() {
            while !p.is_empty() && p[0].new_lineno().is_none() {
                *p = &p[1..];
            }
        }
        if let Some(next_line) = patches
            .iter()
            .filter_map(|(_, lps)| lps.get(0).map(|p| p.new_lineno().unwrap()))
            .min()
        {
            let mut author_ofs = sum;
            for ((_, p), ofs) in patches.iter_mut().zip(1..) {
                if p.get(0)
                    .map(|lp| lp.new_lineno().unwrap())
                    .map_or(false, |lno| lno == next_line)
                {
                    author_ofs -= ofs;
                    *p = &p[1..];
                }
            }
            if author_ofs > 0 && author_ofs <= patches.len() {
                ret.push(Addition {
                    line: next_line as u32,
                    author: patches[author_ofs as usize - 1].0,
                });
            } else {
                ret.push(Addition {
                    author: merger,
                    line: next_line as u32,
                })
            }
        } else {
            break;
        }
    }
    ret
}

impl<'a> Tree<'a> {
    pub fn empty() -> Self {
        Tree {
            root: HashMap::new(),
        }
    }

    pub fn from_commit<'b>(commit: &'b GitCommit<'b>, author: u32) -> Self {
        let empty = Self::empty();
        let es = [&empty];
        let diff = commit.diff_with(vec![commit.scratch()].iter()).unwrap();

        Self::analyze_patch(&es, diff.as_ref(), author)
    }

    fn copy_from_old_tree(
        &mut self,
        other: &Self,
        old: Option<&Path>,
        new: Option<&Path>,
    ) -> Option<&mut Vec<LineBlock>> {
        match (old, new) {
            (Some(old), Some(new)) if old != new => {
                self.root.remove(old);
                self.root.insert(new.to_owned(), other.root[old].clone());
            }
            (Some(_old), Some(_new)) => {}
            (_, Some(new)) => {
                self.root.insert(new.to_owned(), Cow::Owned(vec![]));
            }
            _ => {
                if let Some(old) = old {
                    self.root.remove(old);
                }
            }
        }
        if let Some(path) = new {
            if !self.root.contains_key(path) {
                self.root.insert(path.to_owned(), Cow::Owned(vec![]));
            }
            self.root.get_mut(path).map(|cell| cell.to_mut())
        } else {
            None
        }
    }

    pub fn analyze_patch(trees: &[&Self], patch: &[TreePatch], merger: u32) -> Tree<'a> {
        let files = get_patch_filename_list(patch);
        let authors = get_related_authors(patch);
        let mut file_iters: Vec<_> = patch.iter().map(|x| x.files().iter().peekable()).collect();

        let mut ret = trees[0].clone();

        for (old, new) in files {
            let mut file_patch = Vec::with_capacity(patch.len());
            for iter in file_iters.iter_mut() {
                while iter.peek().map_or(false, |&p| p.new_path() < new) {
                    iter.next();
                }
                if iter.peek().map_or(false, |&f| f.new_path() == new) {
                    file_patch.push(iter.next());
                } else {
                    file_patch.push(None);
                }
            }

            if let Some(patch) = file_patch[0] {
                let mut patch = patch.patch[..].iter().peekable();
                if let Some(file) = ret.copy_from_old_tree(&trees[0], old, new) {
                    let mut new_base = 0;
                    let mut old_base = 0;
                    for block in file.iter_mut() {
                        let mut new_size = block.size;
                        while patch.peek().map_or(false, |line_diff| {
                            line_diff.old_lineno().unwrap_or(0) < old_base + block.size
                                && line_diff.new_lineno().unwrap_or(0) < new_base + new_size
                        }) {
                            let line_diff = patch.next().unwrap();

                            if let Some(line) = line_diff.old_lineno() {
                                if line > old_base {
                                    new_size -= 1;
                                }
                            } else if let Some(line) = line_diff.new_lineno() {
                                if line > new_base {
                                    new_size += 1;
                                }
                            }
                        }
                        old_base += block.size;
                        new_base += new_size;
                        block.size = new_size;
                    }
                    for item in patch {
                        if item.new_lineno().is_some() {
                            if file.is_empty() {
                                file.push(LineBlock {
                                    author_id: merger,
                                    size: 0,
                                });
                            }
                            file.last_mut().unwrap().size += 1;
                        }
                    }
                }
            }

            let patch_iter = authors.iter().map(|x| *x).zip(file_patch.into_iter());

            let merged_diff = merge_file_patch(patch_iter, merger);

            if let Some(file) = new.map(|p| ret.root.get_mut(p)).flatten() {
                let mut idx = 0;
                let mut base = 0;
                let mut buffer = vec![];
                for block in file.iter() {
                    let mut last_begin = base;
                    let last_end = base + block.size;
                    while idx < merged_diff.len() && merged_diff[idx].line < last_end {
                        if last_begin < merged_diff[idx].line {
                            buffer.push(LineBlock {
                                author_id: block.author_id,
                                size: merged_diff[idx].line - last_begin,
                            });
                        }
                        buffer.push(LineBlock {
                            author_id: merged_diff[idx].author,
                            size: 1,
                        });
                        last_begin = merged_diff[idx].line + 1;
                        idx += 1;
                    }
                    if last_begin < last_end {
                        buffer.push(LineBlock {
                            author_id: block.author_id,
                            size: last_end - last_begin,
                        });
                    }
                    base += block.size;
                }

                let mut j = 1;
                for i in 1..buffer.len() {
                    if buffer[j - 1].author_id == buffer[i].author_id {
                        buffer[j - 1].size += buffer[i].size;
                    } else {
                        buffer[j] = buffer[i].clone();
                        j += 1;
                    }
                }
                buffer.resize(
                    j,
                    LineBlock {
                        author_id: 0,
                        size: 0,
                    },
                );
                *file = Cow::Owned(buffer);
            }
        }

        ret
    }

    pub fn stat<Predit: Fn(&Path) -> bool>(&self, predict: Predit) -> Vec<u32> {
        let mut ret = vec![];
        for (path, file) in self.root.iter() {
            if !predict(path) {
                continue;
            }
            for block in file.as_ref() {
                if ret.len() < block.author_id as usize + 1 {
                    ret.resize(block.author_id as usize + 1, 0);
                }
                ret[block.author_id as usize] += block.size;
            }
        }
        ret
    }
}
