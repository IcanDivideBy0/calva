use std::collections::BTreeSet;

pub struct IdGenerator {
    pub next: u16,
    recycle: BTreeSet<u16>,
}

impl IdGenerator {
    pub fn new(init: u16) -> Self {
        Self {
            recycle: BTreeSet::new(),
            next: init,
        }
    }

    pub fn count(&self) -> u16 {
        self.next
    }

    pub fn get(&mut self) -> u16 {
        self.recycle.pop_first().unwrap_or_else(|| {
            self.next += 1;
            self.next - 1
        })
    }

    pub fn recycle(&mut self, id: u16) {
        self.recycle.insert(id);
    }
}
