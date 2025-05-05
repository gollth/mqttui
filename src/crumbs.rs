use ratatui::{prelude::*, widgets::Block};

pub struct Crumbs<'a> {
    index: Option<usize>,
    count: usize,
    block: Block<'a>,
}

impl<'a> Crumbs<'a> {
    pub fn new(index: Option<usize>, count: usize) -> Self {
        Self {
            index,
            count,
            block: Default::default(),
        }
    }

    pub(crate) fn block(mut self, block: Block<'a>) -> Self {
        self.block = block;
        self
    }
}

impl Widget for Crumbs<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let inner = self.block.inner(area);
        self.block.render(area, buffer);
        let count = self.count;

        let label = self
            .index
            .map(|i| format!("{} of {count}", i + 1))
            .unwrap_or("latest".into());
        let label_len = label.chars().count() as u16;
        buffer.set_string(
            inner.width.saturating_sub(label_len),
            inner.y,
            label,
            Style::new().fg(Color::DarkGray).italic(),
        );

        let bound = inner.width - label_len.max(12);
        for x in (0..=bound).take(self.count) {
            let offset = x as i16
                - self
                    .index
                    .map(|i| self.count - 1 - i)
                    .map(|i| i as u16)
                    .unwrap_or_default()
                    .min(bound) as i16;
            let (symbol, color) = match offset.abs() {
                0 => ("●", Color::White),
                1 => ("•", Color::Gray),
                2.. => ("·", Color::DarkGray),
                _ => ("", Color::White),
            };
            buffer.set_string(inner.x + x, inner.y, symbol, color);
        }
    }
}
