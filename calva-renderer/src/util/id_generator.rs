use std::collections::HashSet;

pub struct IdGenerator {
    pub next: u16,
    recycle: HashSet<u16>,
}

impl IdGenerator {
    pub fn new(init: u16) -> Self {
        Self {
            recycle: HashSet::new(),
            next: init,
        }
    }

    pub fn count(&self) -> u16 {
        self.next
    }

    pub fn get(&mut self) -> u16 {
        if let Some(recycled) = self.recycle.iter().next().copied() {
            self.recycle.remove(&recycled);
            recycled
        } else {
            self.next += 1;
            self.next - 1
        }
    }

    pub fn recycle(&mut self, id: u16) {
        self.recycle.insert(id);
    }
}
