//! VT520/VT525 parts of the memory model.

/// Bank bits: P1.6-P1.4.
pub(super) fn rom_bank(p1: u8) -> u8 {
    (p1 >> 4) & 0b111
}
