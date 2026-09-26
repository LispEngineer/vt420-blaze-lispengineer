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
}
