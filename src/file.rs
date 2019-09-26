use crate::author::AuthorStat;

pub struct RepoFile(Vec<u16>);
impl RepoFile {
    pub fn iter(&self) -> impl Iterator<Item = &u16> {
        self.0.iter()
    }
    pub fn empty() -> Self {
        RepoFile(vec![])
    }
    pub fn update(
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
                    // old_pos might be out of bound if the file
                    // is previously changed form a bindary to a text
                    if self.0.len() > old_pos {
                        stat.incrment_author(self.0[old_pos], -1);
                    }
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
