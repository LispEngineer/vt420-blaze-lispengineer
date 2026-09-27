use tracing::trace;

/// Compute the 4-bit value read from 0x7ff6.
///
/// 0x7ff6 is a small accumulator in the DC7166 that latches once per frame.
///
/// We can model the accumulator as a pixel counter, but this is not a full
/// "grounded in reality" model.
pub(crate) fn calculate_7ff6_read(mapper3: u8, mapper4: u8, vram: &[u8]) -> u8 {
    // Physical
    const FONT_ROWS: u8 = 15; // rows not including underline
    const BOLD_WEIGHT: u8 = 2; // bold "double-strikes" the body; TODO: should blink
    const FONT_COLS_132: u8 = 6;
    const FONT_COLS_80: u8 = 10;

    // Magic (not yet derived from first principles)
    const STATUS_UNDERLINE: u8 = 9;
    const DIVIDER_LINE: u8 = 1;
    const DH_BOTTOM_ADJUSTMENT: u8 = 7;
    const DW_ADJUSTMENT: u8 = 10;

    // mapper3 (0x7ff3): bit6 = blink, bit3 = screen select, bit1 = s1 invert, bit0 = s1 132
    // mapper4 (0x7ff4): bit3 = mystery, bit1 = s2 invert, bit0 = s2 132
    let screen2_selected = mapper3 & 0x08 != 0;
    let blink = mapper3 & 0x40 != 0;
    let mystery = mapper4 & 0x08 != 0;

    let s1_is_132 = mapper3 & 0x01 != 0;
    let s1_invert = mapper3 & 0x02 != 0;
    let s2_is_132 = mapper4 & 0x01 != 0;
    let s2_invert = mapper4 & 0x02 != 0;

    let (is_132, invert) = if screen2_selected {
        (s2_is_132, s2_invert)
    } else {
        (s1_is_132, s1_invert)
    };

    // Screen flip / window split: row attr bit 1 marks the split point.
    let mut flip_row = 26usize;
    for r in 0..26 {
        let t = r * 2;
        if t + 1 >= vram.len() {
            break;
        }
        if vram[t + 1] & 0x02 != 0 {
            flip_row = r;
            break;
        }
    }
    let has_flip = flip_row < 26;

    let mut total: u8 = 0;
    let mut chars = [0u16; 136];

    for r in 0..26usize {
        let t = r * 2;
        if t + 1 >= vram.len() {
            break;
        }
        let addr_byte = vram[t];
        let row_attr = vram[t + 1];

        // Row geometry field (attr bits 2..3): 1 = double-width, 2 = DH-top, 3 = DH-bottom.
        let geom = (row_attr >> 2) & 3;
        let dh_top = geom == 2;
        let dh_bottom = geom == 3;
        let double_width = geom == 1;

        let row_is_132 = if has_flip && r >= flip_row {
            s2_is_132
        } else {
            is_132
        };

        // Status row: contributes its own underline (removed under DH-top) + divider.
        if r == 25 {
            let st = if dh_top { 0 } else { STATUS_UNDERLINE };
            let divider = if has_flip && r >= flip_row {
                DIVIDER_LINE
            } else {
                0
            };
            total = total.wrapping_add(st).wrapping_add(divider);
            continue;
        }

        if addr_byte == 0 {
            continue;
        }
        let row_offset = ((addr_byte >> 1) as usize) << 8;
        if row_offset + 256 > vram.len() {
            continue;
        }

        decode_row_chars(&vram[row_offset..row_offset + 256], &mut chars);

        let n = if row_is_132 { 132 } else { 80 }.min(chars.len());
        let cellpx: u8 = if row_is_132 {
            FONT_COLS_132
        } else {
            FONT_COLS_80
        };

        // Unclear why these are needed
        let (block_edge, right_edge_clip): (u8, u8) = if dh_top {
            (0, 0)
        } else if row_is_132 {
            (3, 2)
        } else {
            (0, 3)
        };

        // Solid-fill body of one lit cell.
        let lit_body = FONT_ROWS.wrapping_mul(cellpx).wrapping_mul(BOLD_WEIGHT);

        let mut row_sum: u8 = 0;
        for i in 0..n {
            // All cells are underlined but underline is never bold (!)
            row_sum = row_sum.wrapping_add(cellpx);

            // Note: all chars inverted in the POST, the NULL cell is the LIT
            // rectangle.
            let is_block = chars[i] & 0x100 != 0;
            if is_block {
                row_sum = row_sum.wrapping_add(lit_body);
            } else {
                let mut edge = block_edge;
                if i == n - 1 {
                    edge = edge.wrapping_sub(right_edge_clip); // clipped at right screen edge
                }
                row_sum = row_sum.wrapping_add(edge);
            }
        }

        // EMPIRICAL — vertical-emission TODO. DH-bottom doubles the underline onto
        // the bottom half; double-width re-renders at 2x. Both are vertical effects
        // this horizontal scan can't derive, and Dataset B can't separate them.
        if dh_bottom {
            row_sum = row_sum.wrapping_add(DH_BOTTOM_ADJUSTMENT);
        } else if double_width {
            row_sum = row_sum.wrapping_add(DW_ADJUSTMENT);
        }

        total = total.wrapping_add(row_sum);
    }

    // mystery: status-row bold/blink control -> captured status values.
    if mystery {
        total = total.wrapping_add(myst_lookup(is_132, invert, blink));
    }

    let result = total & 0x0f;

    trace!(
        "7ff6: m3={:02X} m4={:02X} is132={} inv={} bl={} myst={} flip={} -> {:02X}",
        mapper3, mapper4, is_132, invert, blink, mystery, flip_row, result
    );

    result
}

/// `mystery` (0x7ff4 bit 3) toggles bold/blink on the STATUS line (in theory).
///
/// TODO: Modelled as a lookup table until we can figure it out.
fn myst_lookup(is_132: bool, invert: bool, blink: bool) -> u8 {
    match (is_132, invert, blink) {
        (false, false, false) => 0,
        (false, false, true) => 2,
        (false, true, false) => 9,
        (false, true, true) => 2,
        (true, false, false) => 0,
        (true, false, true) => 10,
        (true, true, false) => 14,
        (true, true, true) => 10,
    }
}

/// Duplicated (but simplified) code from video.rs.
fn decode_row_chars(page: &[u8], chars: &mut [u16]) {
    let mut b: u16 = 0;
    let mut char: usize = 0;
    for i in 0..108 {
        let cb = page[i];
        match i % 3 {
            0 => b = cb as u16,
            1 => {
                b |= ((cb & 0xf) as u16) << 8;
                chars[char] = b;
                char += 1;
                b = ((cb & 0xf0) as u16) >> 4;
            }
            _ => {
                b |= (cb as u16) << 4;
                chars[char] = b;
                char += 1;
            }
        }
    }
    for i in 128..221 {
        let cb = page[i];
        let ii = i + 1;
        match ii % 3 {
            0 => b = cb as u16,
            1 => {
                b |= ((cb & 0xf) as u16) << 8;
                chars[char] = b;
                char += 1;
                b = ((cb & 0xf0) as u16) >> 4;
            }
            _ => {
                b |= (cb as u16) << 4;
                chars[char] = b;
                char += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::calculate_7ff6_read;
    use hex_literal::hex;

    fn initialize_checkerboard(vram: &mut [u8]) {
        // The offsets for each row - remember that this is shifted left by 1
        // when stored in ram. This appears to be a bit sweep: single bits,
        // double bits, triple bits, alternating, etc.
        const ROWS: [u8; 27] = hex!(
            "01 02 04 08 05 10 20 40 50 70 11 22 44 2a 55 03 06 0c 18 30 60 07 0e 1c 38 0f 1e"
        );

        const EVEN_VRAM_ROW: &[u8] = &hex![
            r"
            00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0
            00 0E E0 00 0E E0 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 
            98 8F F9 98 8F F9 98 8F F9 98 8F F9 00 0E E0 00 0E E0 00 0E E0 00 0E E0 
            00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 98 8F F9 98 8F F9 
            98 8F F9 98 8F F9 98 8F F9 98 8F F9 00 00 00 00 00 00 00 00 00 00 00 00 
            00 00 00 00 00 00 00 00 98 8F F9 98 8F F9 98 8F F9 98 8F F9 00 0E E0 00 
            0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 
            0E E0 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 
            8F F9 98 8F F9 98 8F F9 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 
            0E E0 00 00 00 54 55 55 55 55 55 55 55 55 55 55 55 55 55 55 55 55 55 55 
            55 55 55 55 55 55 55 55 55 55 55 55 55 55 01 00
        "
        ];

        const ODD_VRAM_ROW: &[u8] = &hex![
            r"
            98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9
            98 8F F9 98 8F F9 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0
            00 0E E0 00 0E E0 00 0E E0 00 0E E0 98 8F F9 98 8F F9 98 8F F9 98 8F F9
            98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 00 0E E0 00 0E E0
            00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 00 00 00 00 00 00 00 00 00 00 00
            00 00 00 00 00 00 00 00 00 0E E0 00 0E E0 00 0E E0 00 0E E0 98 8F F9 98
            8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98
            8F F9 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00 0E E0 00
            0E E0 00 0E E0 00 0E E0 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98 8F F9 98
            8F F9 00 00 00 54 55 55 55 55 55 55 55 55 55 55 55 55 55 55 55 55 55 55
            55 55 55 55 55 55 55 55 55 55 55 55 55 55 01 00
        "
        ];

        for i in 0..27 {
            vram[i * 2] = ROWS[i] << 1;
            let page = ROWS[i] as usize * 256;
            if (i / 5) % 2 == 0 {
                vram[page..page + 256].copy_from_slice(EVEN_VRAM_ROW);
            } else {
                vram[page..page + 256].copy_from_slice(ODD_VRAM_ROW);
            }
        }
    }

    /// Test the impact of the 7ff3 and 7ff4 registers on the 7ff6 register.
    #[test]
    fn test_calculate_mapper_7ff6_a() {
        let mut vram = Vec::with_capacity(0x10000);
        vram.resize(0x10000, 0);
        initialize_checkerboard(&mut vram);

        const EXPECTED: [u8; 32] = hex!(
            "
            0b 0b 0b 0d 0b 04 0b 0d
            03 03 03 0d 03 01 03 0d
            0b 0b 0b 0d 0b 04 0b 0d
            03 03 03 0d 03 01 03 0d
        "
        );
        let mut results = [0u8; 32];
        let mut mapper3 = 0;
        let mut mapper4 = 0;

        // Bits (high-to-low): Screen, 80/132, Invert, Blink, Mystery
        for i in 0..32 {
            let i2 = (i & (1 << 2)) != 0;
            let i3 = (i & (1 << 3)) != 0;
            mapper3 &= 0b10111111;
            if (i & (1 << 1)) != 0 {
                mapper3 |= 0b01000000;
            }
            mapper3 |= 0b00001000;
            if (i & (1 << 4)) != 1 {
                mapper3 = (mapper3 & 0b11110100) | (i3 as u8) | ((i2 as u8) << 1);
            }
            mapper4 &= 0b11110111;
            if (i & (1 << 0)) != 0 {
                mapper4 |= 0b00001000;
            }
            if (i & (1 << 4)) != 0 {
                mapper4 = (mapper4 & 0b11111100) | (i3 as u8) | ((i2 as u8) << 1);
            }

            results[i] = calculate_7ff6_read(mapper3, mapper4, &vram);
        }

        // This test does screen A + screen B but we only need to dump the first 16 values
        // since they are implemented identically.
        if results != EXPECTED {
            eprintln!("| 132 | Inv | Blnk | Myst | Exp | Act | Δ   |");
            eprintln!("|-----|-----|------|------|-----|-----|-----|");
            for mut i in 0..16 {
                let idx = i;
                let mystery = i & 1;
                i >>= 1;
                let blink = i & 1;
                i >>= 1;
                let inv = i & 1;
                i >>= 1;
                let is_132 = i & 1;
                eprintln!(
                    "| {}   |  {}  |  {}   |  {}   | {:02X?}  | {:02X?}  | {:<+3} |",
                    is_132,
                    inv,
                    blink,
                    mystery,
                    EXPECTED[idx],
                    results[idx],
                    EXPECTED[idx].wrapping_sub(results[idx]) % 16
                );
            }

            assert_eq!(results, EXPECTED, "Mismatch!");
        }
    }

    /// Test the impact of row attributes on the 7ff6 register.
    #[test]
    fn test_calculate_mapper_7ff6_b() {
        let mut vram = Vec::with_capacity(0x10000);
        vram.resize(0x10000, 0);
        initialize_checkerboard(&mut vram);

        // Set the second field of all rows to 0x0c, 0x08, 0x04, 0x00 Config: s1
        // selected (80col, no inv), mystery on. myst_effect = 0 (no blink, no
        // inv). We only modify the row table attributes (not the char data) so
        // the checkerboard pattern remains intact for the full scan.
        const EXPECTED: [u8; 4] = hex!("0a 00 05 0b");
        const ROW_ATTRS: [u8; 4] = [0x0c, 0x08, 0x04, 0];
        let mapper3 = 4;
        let mapper4 = 0x1b;
        let mut results = [0u8; 4];

        for (i, &v) in ROW_ATTRS.iter().enumerate() {
            for r in 0..26 {
                vram[r * 2 + 1] = v;
            }

            results[i] = calculate_7ff6_read(mapper3, mapper4, &vram);
        }

        if results != EXPECTED {
            eprintln!("| Case         | Expected | Actual | Δ   | ");
            eprintln!("|--------------|----------|--------|-----|");
            let cases = ["Normal", "DW", "DH Top", "DH Bottom"];
            for i in 0..4 {
                eprintln!(
                    "| {:<12} | {:02X?}       | {:02X?}     | {:<+3} |",
                    cases[i],
                    EXPECTED[i],
                    results[i],
                    EXPECTED[i].wrapping_sub(results[i]) % 16
                );
            }

            panic!("Mismatch!");
        }
    }

    /// Uses a flip flags to scan _up_ the screen, testing the impact of the
    /// screen flip/divider line. The flip flag (bit 1 of row attr) toggles rows
    /// from s1 to s2 config.
    #[test]
    fn test_calculate_mapper_7ff6_c() {
        let mut vram = Vec::with_capacity(0x10000);
        vram.resize(0x10000, 0);
        initialize_checkerboard(&mut vram);

        // Set bit 1 of a single field at a time, starting from the second last
        // (ie: 0x0f in the list of ROWS in `initialize_checkerboard`). Config:
        // s1 selected (80col, no inv), s2 has 132col+inv. Mystery and blink
        // flags clear.

        // Note: The increments (2*5, 1*5, 2*5, 1*5, 2*5) match the checkerboard
        // row heights! This is part of the hint that helps us decode 7ff6 as a
        // font pixel counter.
        const EXPECTED: [u8; 26] =
            hex!("04 06 08 0a 0c 0e 0f 00 01 02 03 05 07 09 0b 0d 0e 0f 00 01 02 04 06 08 0a 0c");
        let mapper3 = 0x04; // s1 selected, 80col, no inv
        let mapper4 = 0x13; // s2: 132col, inv. bit 4 set (no mystery).
        let mut results = [0u8; 26];
        for i in (0..26).rev() {
            vram[i * 2 + 1] ^= 2;
            vram[i * 2 + 3] = 0;

            results[i] = calculate_7ff6_read(mapper3, mapper4, &vram);
        }

        if results != EXPECTED {
            eprintln!("| Case         | Expected | Actual | Δ   | ");
            eprintln!("|--------------|----------|--------|-----|");
            for i in 0..26 {
                let case = format!("Row {}", i);
                eprintln!(
                    "| {:<12} | {:02X?}       | {:02X?}     | {:<+3} |",
                    case,
                    EXPECTED[i],
                    results[i],
                    EXPECTED[i].wrapping_sub(results[i]) % 16
                );
            }

            assert_eq!(results, EXPECTED);
        }
    }
}
