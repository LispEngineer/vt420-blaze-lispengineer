use std::ops::{BitOr, BitOrAssign};

pub const FRAME_WIDTH: usize = 800;
pub const FRAME_HEIGHT: usize = 416;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextAttr(u8);

impl TextAttr {
    pub const NONE: Self = Self(0);
    pub const BOLD: Self = Self(0x01);
    pub const UNDERLINE: Self = Self(0x02);
    pub const REVERSE: Self = Self(0x04);
    pub const BLINK: Self = Self(0x08);
    pub const LEFT_HALF: Self = Self(0x10);
    pub const RIGHT_HALF: Self = Self(0x20);
    pub const TOP_HALF: Self = Self(0x40);
    pub const BOTTOM_HALF: Self = Self(0x80);

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for TextAttr {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for TextAttr {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineSize {
    #[default]
    Single,
    DoubleWidth,
    DoubleHeightTop,
    DoubleHeightBottom,
}

impl LineSize {
    pub fn is_double_width(self) -> bool {
        self != LineSize::Single
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextLine {
    pub size: LineSize,
    pub columns: usize,
    pub reverse: bool,
}

pub trait Display {
    fn render_framebuffer(&self, frame: &mut [u8]);

    fn render_textbuffer(
        &self,
        line: &mut dyn FnMut(usize, TextLine),
        cell: &mut dyn FnMut(usize, usize, char, TextAttr),
    );
}

pub fn text_grid<D: Display + ?Sized>(display: &D) -> Vec<Vec<(char, TextAttr)>> {
    let mut grid: Vec<Vec<(char, TextAttr)>> = vec![];
    display.render_textbuffer(&mut |_, _| {}, &mut |row, column, ch, attr| {
        if grid.len() <= row {
            grid.resize(row + 1, vec![]);
        }
        let cells = &mut grid[row];
        if cells.len() <= column {
            cells.resize(column + 1, (' ', TextAttr::NONE));
        }
        cells[column] = (ch, attr);
    });
    grid
}

pub fn text_lines<D: Display + ?Sized>(display: &D) -> Vec<String> {
    text_grid(display)
        .into_iter()
        .map(|row| {
            let s: String = row
                .iter()
                .filter(|(_, attr)| !attr.contains(TextAttr::RIGHT_HALF))
                .map(|&(c, _)| c)
                .collect();
            s.trim_end().to_string()
        })
        .collect()
}

pub fn render_frame<D: Display + ?Sized>(display: &D) -> Vec<u8> {
    let mut frame = vec![0u8; FRAME_WIDTH * FRAME_HEIGHT * 4];
    display.render_framebuffer(&mut frame);
    frame
}

#[cfg(feature = "vram-dump")]
pub fn save_png<D: Display + ?Sized>(display: &D, path: &std::path::Path) -> std::io::Result<()> {
    write_png(&render_frame(display), path)
}

#[cfg(feature = "vram-dump")]
pub fn write_png(frame: &[u8], path: &std::path::Path) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, FRAME_WIDTH as u32, FRAME_HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(frame))
        .map_err(std::io::Error::other)
}
