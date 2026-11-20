// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Lossless storage for small 8-bit glyphs.
//! The first two bytes hold the original pixel count in little-endian order.
//! Each group of 32 pixels stores a little-endian coverage mask and the distance from the end of the buffer to its first alpha byte.
//! A set bit's rank within its mask indexes the alpha bytes without scanning earlier pixels.

use alloc::{vec, vec::Vec};

const GROUP: usize = 32;
const ENTRY: usize = 5;

/// Returns packed bytes only when the result is smaller than the raw alpha map.
pub fn encode(pixels: &[u8]) -> Option<Vec<u8>> {
    let len = u16::try_from(pixels.len()).ok()?;
    let nonzero = u8::try_from(pixels.iter().filter(|&&p| p != 0).count()).ok()?;
    let header = 2 + ENTRY * pixels.len().div_ceil(GROUP);
    let encoded_len = header + usize::from(nonzero);
    if encoded_len >= pixels.len() {
        return None;
    }
    let mut bytes = vec![0; header];
    bytes[..2].copy_from_slice(&len.to_le_bytes());
    for (block, group) in pixels.chunks(GROUP).enumerate() {
        let start = 2 + ENTRY * block;
        bytes[start + 4] = (encoded_len - bytes.len()) as u8;
        for (bit, &value) in group.iter().enumerate() {
            if value != 0 {
                bytes[start + bit / 8] |= 1 << (bit % 8);
                bytes.push(value);
            }
        }
    }
    Some(bytes)
}

/// Returns the number of pixels in a packed glyph.
pub fn pixel_count(data: &[u8]) -> usize {
    u16::from_le_bytes([data[0], data[1]]) as usize
}

/// Reads one pixel from the output of [`encode`].
pub fn alpha_at(data: &[u8], pos: usize) -> u8 {
    let block = 2 + ENTRY * (pos / GROUP);
    let mask = u32::from_le_bytes(data[block..block + 4].try_into().unwrap());
    let bit = 1u32 << (pos % GROUP);
    if mask & bit == 0 {
        return 0;
    }
    data[data.len() - data[block + 4] as usize + (mask & (bit - 1)).count_ones() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pixel_round_trips_across_group_boundaries() {
        for len in 1..=255 {
            for phase in 0..7 {
                let raw: Vec<_> = (0..len)
                    .map(|i| if (i + phase) % 7 < 2 { (i as u8).wrapping_mul(19) } else { 0 })
                    .collect();
                if let Some(packed) = encode(&raw) {
                    assert!(packed.len() < len);
                    assert_eq!(pixel_count(&packed), len);
                    for (pos, expected) in raw.into_iter().enumerate() {
                        assert_eq!(alpha_at(&packed, pos), expected);
                    }
                } else {
                    assert!(
                        2 + ENTRY * len.div_ceil(GROUP) + raw.iter().filter(|&&x| x != 0).count()
                            >= len
                    );
                }
            }
        }
        assert!(encode(&[]).is_none());
        assert!(encode(&[255; 128]).is_none());
        let mut sparse = vec![0; 928];
        for pos in (0..928).step_by(4) {
            sparse[pos] = 255;
        }
        let packed = encode(&sparse).unwrap();
        assert_eq!(pixel_count(&packed), 928);
        assert!(sparse.iter().enumerate().all(|(i, &v)| alpha_at(&packed, i) == v));
        assert!(encode(&vec![0; 65536]).is_none());
        assert!(encode(&[0, 111].repeat(256)).is_none());
    }
}
