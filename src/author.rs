use std::collections::HashMap;

pub struct AuthorIndex {
    name2id: HashMap<String, u16>,
    id2name: Vec<String>,
}

impl AuthorIndex {
    pub fn new() -> Self {
        Self {
            name2id: HashMap::new(),
            id2name: vec![],
        }
    }

    pub fn get_id_by_name(&mut self, name: &str) -> u16 {
        if !self.name2id.contains_key(name) {
            let idx = self.id2name.len();
            self.id2name.push(name.to_string());
            self.name2id.insert(name.to_string(), idx as u16);
        }
        self.name2id[name] as u16
    }

    pub fn get_name_by_id(&self, idx: u16) -> Option<&str> {
        if idx as usize >= self.id2name.len() {
            return None;
        }
        Some(self.id2name[idx as usize].as_ref())
    }

    #[allow(dead_code)]
    pub fn num_authors(&self) -> usize {
        self.id2name.len()
    }
}

pub struct AuthorStat(Vec<usize>);

impl AuthorStat {
    pub fn new() -> Self {
        AuthorStat(vec![])
    }

    pub fn stats(&self) -> &[usize] {
        &self.0[..]
    }

    #[allow(dead_code)]
    pub fn print(&self, a: &AuthorIndex) {
        for (i, c) in (0..).zip(self.0.iter()) {
            println!("{} {}", a.get_name_by_id(i).unwrap(), c);
        }
    }
    pub fn incrment_author(&mut self, author: u16, delta: i32) {
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
