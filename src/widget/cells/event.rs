pub enum CellGridEvent {
    SetHighlightedPosition(usize, usize),
    ConsumeInput,
    Multiple(Vec<CellGridEvent>),
}
