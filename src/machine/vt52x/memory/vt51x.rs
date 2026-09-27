//! VT510 parts of the memory model.

/// Bank bits: P1.7 = A16, P1.6 = A17, P1.5 = A18.
pub(super) fn rom_bank(p1: u8) -> u8 {
    (p1 >> 7) & 1 | (p1 >> 5) & 2 | (p1 >> 3) & 4
}
