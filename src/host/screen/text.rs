use std::cell::Cell;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use crate::machine::generic::display::{Display, TextAttr, TextLine};

pub struct TextScreen<'a, D: Display + ?Sized> {
    display: &'a D,
}

impl<'a, D: Display + ?Sized> TextScreen<'a, D> {
    pub fn new(display: &'a D) -> Self {
        Self { display }
    }
}

impl<D: Display + ?Sized> Widget for TextScreen<'_, D> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let current = Cell::new(TextLine::default());
        let style = |attr: TextAttr| {
            let mut style = Style::default();
            if attr.contains(TextAttr::UNDERLINE) {
                style = style.underlined();
            }
            if attr.contains(TextAttr::BOLD) {
                style = style.bold();
            }
            if attr.contains(TextAttr::REVERSE) {
                style = style.reversed();
            }
            if attr.contains(TextAttr::BLINK) {
                style = style.slow_blink();
            }
            style
        };
        let mut put = |x: usize, y: usize, ch: char, style: Style| {
            if x >= area.width as usize || y >= area.height as usize {
                return;
            }
            if let Some(cell) = buf.cell_mut((area.left() + x as u16, area.top() + y as u16)) {
                cell.set_char(ch);
                cell.set_style(style);
            }
        };
        self.display.render_textbuffer(
            &mut |_, line| current.set(line),
            &mut |row, column, ch, attr| {
                let line = current.get();
                if column == 0 {
                    let blank = if line.reverse {
                        TextAttr::REVERSE
                    } else {
                        TextAttr::NONE
                    };
                    for x in line.columns..132 {
                        put(x, row, ' ', style(blank));
                    }
                }
                let (x, width) = if line.size.is_double_width() {
                    (column * 2, 2)
                } else {
                    (column, 1)
                };
                for i in 0..width {
                    let blank = i > 0 || attr.contains(TextAttr::RIGHT_HALF);
                    put(x + i, row, if blank { ' ' } else { ch }, style(attr));
                }
            },
        );
    }
}
