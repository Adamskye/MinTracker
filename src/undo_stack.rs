use std::collections::VecDeque;

pub struct UndoStack<T> {
    // the head is the current state
    stack: VecDeque<T>,
}

impl<T> UndoStack<T>
where
    T: Clone + PartialEq,
{
    pub fn new(max_depth: usize) -> Self {
        UndoStack {
            stack: VecDeque::with_capacity(max_depth),
        }
    }

    pub fn push(&mut self, state: &T) {
        // don't push if the new state is the same as the current state
        if Some(state) == self.stack.back() {
            return;
        }

        if self.stack.len() == self.stack.capacity() {
            self.stack.pop_front();
        }
        self.stack.push_back(state.clone());
    }

    pub fn undo(&mut self) -> Option<T> {
        if self.stack.len() <= 1 {
            return None;
        }

        self.stack.pop_back();
        self.stack.back().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_stack() {
        let mut undo_stack = UndoStack::new(3);
        undo_stack.push(&1);
        undo_stack.push(&2);
        undo_stack.push(&3);
        assert_eq!(undo_stack.undo(), Some(2));
        assert_eq!(undo_stack.undo(), Some(1));
        assert_eq!(undo_stack.undo(), None);
    }
}
