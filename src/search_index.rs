use crate::syntax_highlighting::HighlightType;

pub enum SearchDirection {
    Forward,
    Backward,
}

pub struct SearchIndex {
    pub x_index: usize,
    pub y_index: usize,
    pub x_direction: Option<SearchDirection>,
    pub y_direction: Option<SearchDirection>,
    pub previous_highlight: Option<(usize, Vec<HighlightType>)>,
}

impl SearchIndex {
    pub fn new() -> Self {
        Self {
            x_index: 0,
            y_index: 0,
            x_direction: None,
            y_direction: None,
            previous_highlight: None,
        }
    }

    pub fn reset(&mut self) {
        self.y_index = 0;
        self.x_index = 0;
        self.y_direction = None;
        self.x_direction = None;
        self.previous_highlight = None
    }
}
