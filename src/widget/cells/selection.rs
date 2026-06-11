#[derive(Clone, Debug)]
pub struct GridSelection {
    // (row, column)
    pub first: (usize, usize),
    pub last: (usize, usize),
}

impl GridSelection {
    pub fn small_row(&self) -> usize {
        self.first.0.min(self.last.0)
    }

    pub fn big_row(&self) -> usize {
        self.first.0.max(self.last.0)
    }

    pub fn small_col(&self) -> usize {
        self.first.1.min(self.last.1)
    }

    pub fn big_col(&self) -> usize {
        self.first.1.max(self.last.1)
    }
}
