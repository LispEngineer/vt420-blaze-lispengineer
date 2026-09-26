use std::cell::Cell;

/// VGA-style palette DAC (Bt478/G171 family).
pub struct Ramdac {
    /// 256 entries of 6-bit red, green and blue.
    pub palette: [[u8; 3]; 256],
    pub mask: u8,
    pub loaded: bool,
    write_index: u8,
    write_component: usize,
    read_index: Cell<u8>,
    read_component: Cell<usize>,
}

impl Default for Ramdac {
    fn default() -> Self {
        Self {
            palette: [[0; 3]; 256],
            mask: 0xFF,
            loaded: false,
            write_index: 0,
            write_component: 0,
            read_index: Cell::new(0),
            read_component: Cell::new(0),
        }
    }
}

impl Ramdac {
    pub fn set_write_index(&mut self, index: u8) {
        self.write_index = index;
        self.write_component = 0;
    }

    pub fn set_read_index(&self, index: u8) {
        self.read_index.set(index);
        self.read_component.set(0);
    }

    pub fn write_data(&mut self, value: u8) {
        self.loaded = true;
        self.palette[self.write_index as usize][self.write_component] = value & 0x3F;
        self.write_component += 1;
        if self.write_component == 3 {
            self.write_component = 0;
            self.write_index = self.write_index.wrapping_add(1);
        }
    }

    pub fn read_data(&self) -> u8 {
        let index = self.read_index.get();
        let component = self.read_component.get();
        let value = self.palette[index as usize][component];
        if component == 2 {
            self.read_component.set(0);
            self.read_index.set(index.wrapping_add(1));
        } else {
            self.read_component.set(component + 1);
        }
        value
    }

    /// An entry as 8-bit RGB.
    pub fn rgb(&self, index: u8) -> (u8, u8, u8) {
        let [r, g, b] = self.palette[(index & self.mask) as usize];
        let scale = |v: u8| v << 2 | v >> 4;
        (scale(r), scale(g), scale(b))
    }
}
