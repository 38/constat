use crate::author::AuthorStat;

pub struct RepoFile {
    author: Vec<u16>,
    tracking: bool,
}

impl Drop for RepoFile {
    fn drop(&mut self) {
        if self.tracking {
            panic!("Code bug: unable to drop a tracking file");
        }
    }
}

impl RepoFile {
    pub fn iter(&self) -> impl Iterator<Item = &u16> {
        self.author.iter()
    }
    pub fn empty(tracking: bool) -> Self {
        RepoFile {
            author: vec![],
            tracking,
        }
    }

    pub fn set_tracking_flag(&mut self, flag: bool, stat: Option<&mut AuthorStat>) {
        if let Some(stat) = stat {
            let delta = match (self.tracking, flag) {
                (true, false) => -1,
                (false, true) => 1,
                _ => 0,
            };

            if delta != 0 {
                for author in self.iter() {
                    stat.incrment_author(*author, delta);
                }
            }
        }

        self.tracking = flag;
    }

    pub fn update(
        &mut self,
        author: u16,
        mut patch: impl Iterator<Item = (Option<usize>, Option<usize>)>,
        stat: &mut AuthorStat,
    ) {
        let mut new_file = vec![];

        let mut next_patch = patch.next();
        let mut new_pos = 0;
        let mut old_pos = 0;

        while next_patch.is_some() || old_pos < self.author.len() {
            let patch_applied = match &next_patch {
                Some((Some(old), Some(_new))) if *old == old_pos => {
                    new_file.push(author);
                    if self.tracking {
                        stat.incrment_author(self.author[old_pos], -1);
                        stat.incrment_author(author, 1);
                    }
                    new_pos += 1;
                    old_pos += 1;
                    true
                }
                Some((None, Some(new))) if *new == new_pos => {
                    new_file.push(author);
                    new_pos += 1;
                    if self.tracking {
                        stat.incrment_author(author, 1);
                    }
                    true
                }
                Some((Some(old), None)) if *old == old_pos => {
                    // old_pos might be out of bound if the file
                    // is previously changed form a bindary to a text
                    if self.author.len() > old_pos && self.tracking {
                        stat.incrment_author(self.author[old_pos], -1);
                    }
                    old_pos += 1;
                    true
                }
                Some((None, None)) => true,
                _ => {
                    if self.author.len() > old_pos {
                        new_file.push(self.author[old_pos]);
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

        self.author = new_file;
    }
}
