pub struct IdGenerator {
    pub next: u32,
    recycle: Vec<u32>,
}

impl IdGenerator {
    pub fn new(init: u32) -> Self {
        Self {
            recycle: vec![],
            next: init,
        }
    }

    pub fn get(&mut self) -> u32 {
        self.recycle.pop().unwrap_or_else(|| {
            self.next += 1;
            self.next - 1
        })
    }

    pub fn recycle(&mut self, id: u32) {
        self.recycle.push(id)
    }
}
