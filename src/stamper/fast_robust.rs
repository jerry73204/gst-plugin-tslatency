// Fast and robust timestamp stamper with BCH error correction

use super::traits::{ReaderConfig, StamperConfig, TimestampReader, TimestampStamper};
use gst::{prelude::*, BufferRef, Clock, FlowError};
use gst_video::{prelude::*, VideoFormatFlags, VideoFrameRef};
use once_cell::sync::Lazy;
use std::sync::Arc;

/// Pre-computed BCH(7,4) encoding table for 4-bit values
static BCH_7_4_TABLE: Lazy<Arc<[u8; 16]>> = Lazy::new(|| {
    let mut table = [0u8; 16];
    for i in 0..16 {
        table[i] = compute_bch_7_4_code(i as u8);
    }
    Arc::new(table)
});

fn compute_bch_7_4_code(data: u8) -> u8 {
    // BCH(7,4) code - encodes 4 bits into 7 bits
    // This is a systematic code: data bits in positions 6-3, parity in 2-0
    let d = data & 0xF;

    // Generator matrix for BCH(7,4) - optimized for single-error correction
    // Parity bits calculated using generator polynomial x^3 + x + 1
    let p0 = (d >> 0) ^ (d >> 1) ^ (d >> 3);
    let p1 = (d >> 0) ^ (d >> 2) ^ (d >> 3);
    let p2 = (d >> 1) ^ (d >> 2) ^ (d >> 3);

    (d << 3) | ((p2 & 1) << 2) | ((p1 & 1) << 1) | (p0 & 1)
}

/// BCH(7,4) syndrome decoding table for single-error correction
static BCH_7_4_SYNDROME_TABLE: [u8; 8] = [
    0b0000000, // No error
    0b0000001, // Error in bit 0 (parity)
    0b0000010, // Error in bit 1 (parity)
    0b0000100, // Error in bit 2 (parity)
    0b0001000, // Error in bit 3 (data)
    0b0010000, // Error in bit 4 (data)
    0b0100000, // Error in bit 5 (data)
    0b1000000, // Error in bit 6 (data)
];

fn decode_bch_7_4(code: u8) -> u8 {
    // Extract data bits (positions 6-3)
    let data = (code >> 3) & 0xF;

    // Calculate syndrome
    let received_parity = code & 0x7;
    let expected = compute_bch_7_4_code(data);
    let expected_parity = expected & 0x7;
    let syndrome = received_parity ^ expected_parity;

    if syndrome == 0 {
        // No error
        return data;
    }

    // Single bit error correction using syndrome
    // For BCH(7,4), syndrome directly indicates error position
    let error_mask = match syndrome {
        0b001 => 0b0000001, // Error in p0
        0b010 => 0b0000010, // Error in p1
        0b100 => 0b0000100, // Error in p2
        0b011 => 0b0001000, // Error in d0
        0b110 => 0b0010000, // Error in d1
        0b111 => 0b0100000, // Error in d2
        0b101 => 0b1000000, // Error in d3
        _ => 0,             // Multiple errors, cannot correct
    };

    let corrected = code ^ error_mask;
    (corrected >> 3) & 0xF
}

/// Fast robust stamper with BCH error correction
///
/// Current implementation:
/// - Uses BCH(7,4) error correction codes with single-bit error correction
/// - Encodes full 64-bit timestamp as 16x4-bit nibbles -> 16x7-bit BCH codes = 112 bits
/// - Also includes 8-bit CRC for additional validation
/// - Total: 120 bits encoded
/// - With block_size=4 and no guard pixels, each bit needs 4x4 pixels
/// - 120 bits can be arranged in a 15x8 grid = 60x32 pixels (fits in 64x64)
pub struct FastRobustStamper {
    block_size: u8,
    use_2d_redundancy: bool,
    guard_pixels: u8,
}

/// Encoded timestamp with BCH(7,4) codes
struct EncodedTimestamp64 {
    // 16 BCH(7,4) codes for 64-bit timestamp (16 nibbles)
    bch_codes: [u8; 16],
    // CRC8 for additional validation
    crc8: u8,
}

impl Default for FastRobustStamper {
    fn default() -> Self {
        Self {
            block_size: 4, // 4x4 pixels per bit
            use_2d_redundancy: true,
            guard_pixels: 0, // No guard band to save space
        }
    }
}

impl TimestampStamper for FastRobustStamper {
    fn stamp(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        clock: &Clock,
        config: &StamperConfig,
    ) -> Result<(), FlowError> {
        let timestamp_usecs = clock.time().unwrap().useconds();
        let encoded = self.encode_timestamp_fast(timestamp_usecs);

        self.stamp_pixels_fast(frame, &encoded, config)
    }

    fn name(&self) -> &'static str {
        "fast-robust"
    }

    fn description(&self) -> &'static str {
        "Fast BCH(7,4) error correcting stamper with full 64-bit timestamps"
    }
}

impl FastRobustStamper {
    fn encode_timestamp_fast(&self, timestamp_usecs: u64) -> EncodedTimestamp64 {
        // Encode full 64-bit timestamp
        let mut bch_codes = [0u8; 16];

        // Split 64-bit timestamp into 16 nibbles (4-bit chunks)
        for i in 0..16 {
            let shift = (15 - i) * 4;
            let nibble = ((timestamp_usecs >> shift) & 0xF) as u8;
            // Encode each nibble with BCH(7,4)
            bch_codes[i] = BCH_7_4_TABLE[nibble as usize];
        }

        // Calculate simple CRC8 for additional validation
        let crc8 = self.calculate_crc8(timestamp_usecs);

        EncodedTimestamp64 { bch_codes, crc8 }
    }

    fn calculate_crc8(&self, data: u64) -> u8 {
        let mut crc = 0u8;
        for i in 0..8 {
            let byte = ((data >> (i * 8)) & 0xFF) as u8;
            crc ^= byte;
            for _ in 0..8 {
                if crc & 0x80 != 0 {
                    crc = (crc << 1) ^ 0x07; // CRC-8 polynomial
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }

    fn stamp_pixels_fast(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        encoded: &EncodedTimestamp64,
        config: &StamperConfig,
    ) -> Result<(), FlowError> {
        let fmt = frame.format_info();
        let flags = fmt.flags();

        let pixel_value_white = if flags.contains(VideoFormatFlags::YUV) {
            235
        } else {
            235
        };
        let pixel_value_black = if flags.contains(VideoFormatFlags::YUV) {
            20
        } else {
            20
        };

        let stride = frame.plane_stride()[0] as usize;
        let plane_data = frame.plane_data_mut(0).unwrap();

        let block_size = self.block_size as usize;
        let guard = self.guard_pixels as usize;
        let total_block_size = block_size + guard;

        let x_offset = config.x as usize;
        let y_offset = config.y as usize;

        // Calculate how many bits we can fit in the available space
        let max_blocks_x = (config.width as usize) / total_block_size;
        let max_blocks_y = (config.height as usize) / total_block_size;
        let max_bits = max_blocks_x * max_blocks_y;

        let mut bit_index = 0;

        // Encode all 16 BCH(7,4) codes (112 bits)
        for i in 0..16 {
            let bch_code = encoded.bch_codes[i];
            for bit_pos in 0..7 {
                // 7 bits per BCH code
                if bit_index >= max_bits {
                    return Ok(()); // Stop if we run out of space
                }

                let bit_value = (bch_code >> bit_pos) & 1 == 1;

                // Calculate block position
                let block_x = (bit_index % max_blocks_x) * total_block_size;
                let block_y = (bit_index / max_blocks_x) * total_block_size;

                // Fast fill using optimized memory operations
                let pixel_value = if bit_value {
                    pixel_value_white
                } else {
                    pixel_value_black
                };

                let y_start = y_offset + block_y;
                let y_end = y_start + block_size;
                let x_start = x_offset + block_x;
                let x_end = x_start + block_size;

                for y in y_start..y_end {
                    let row_start = y * stride + x_start;
                    let row_end = row_start + block_size;

                    if row_end <= plane_data.len() {
                        plane_data[row_start..row_end].fill(pixel_value);
                    }
                }

                bit_index += 1;
            }
        }

        // Encode CRC8 (8 bits)
        for bit_pos in 0..8 {
            if bit_index >= max_bits {
                return Ok(());
            }

            let bit_value = (encoded.crc8 >> bit_pos) & 1 == 1;

            let block_x = (bit_index % max_blocks_x) * total_block_size;
            let block_y = (bit_index / max_blocks_x) * total_block_size;

            let pixel_value = if bit_value {
                pixel_value_white
            } else {
                pixel_value_black
            };

            let y_start = y_offset + block_y;
            let y_end = y_start + block_size;
            let x_start = x_offset + block_x;
            let x_end = x_start + block_size;

            for y in y_start..y_end {
                let row_start = y * stride + x_start;
                let row_end = row_start + block_size;

                if row_end <= plane_data.len() {
                    plane_data[row_start..row_end].fill(pixel_value);
                }
            }

            bit_index += 1;
        }

        Ok(())
    }
}

/// Fast robust reader with BCH error correction
pub struct FastRobustReader {
    block_size: u8,
    guard_pixels: u8,
    threshold: u8,
    min_confidence: f32,
}

impl Default for FastRobustReader {
    fn default() -> Self {
        Self {
            block_size: 4,   // Match stamper
            guard_pixels: 0, // Match stamper
            threshold: 128,
            min_confidence: 0.5, // Lower threshold for compression tolerance
        }
    }
}

impl TimestampReader for FastRobustReader {
    fn read(
        &self,
        frame: &VideoFrameRef<&BufferRef>,
        _clock: &Clock,
        config: &ReaderConfig,
    ) -> Result<Option<u64>, FlowError> {
        Ok(self.decode_timestamp_fast(frame, config))
    }

    fn name(&self) -> &'static str {
        "fast-robust"
    }

    fn description(&self) -> &'static str {
        "Fast BCH(7,4) error correcting reader with full 64-bit timestamps"
    }
}

impl FastRobustReader {
    fn decode_timestamp_fast(
        &self,
        frame: &VideoFrameRef<&BufferRef>,
        config: &ReaderConfig,
    ) -> Option<u64> {
        let stride = frame.plane_stride()[0] as usize;
        let plane_data = frame.plane_data(0).unwrap();

        let block_size = self.block_size as usize;
        let guard = self.guard_pixels as usize;
        let total_block_size = block_size + guard;

        let x_offset = config.x as usize;
        let y_offset = config.y as usize;

        // Calculate how many bits we can fit in the available space
        let max_blocks_x = (config.width as usize) / total_block_size;
        let max_blocks_y = (config.height as usize) / total_block_size;
        let max_bits = max_blocks_x * max_blocks_y;

        let mut bch_codes = [0u8; 16];
        let mut total_confidence = 0f32;
        let mut bit_index = 0;

        // Read 16 BCH(7,4) codes (112 bits)
        for code_idx in 0..16 {
            let mut code_bits = 0u8;

            for bit_pos in 0..7 {
                if bit_index >= max_bits {
                    return None;
                }

                // Calculate block position
                let block_x = (bit_index % max_blocks_x) * total_block_size;
                let block_y = (bit_index / max_blocks_x) * total_block_size;

                // Sample center pixels
                let sample_y = y_offset + block_y + block_size / 2;
                let sample_x = x_offset + block_x + block_size / 2;

                let mut sum = 0u32;
                let mut count = 0u32;

                // Sample 2x2 center pixels for smaller blocks
                for dy in 0..2.min(block_size / 2) {
                    for dx in 0..2.min(block_size / 2) {
                        let y = sample_y + dy;
                        let x = sample_x + dx;
                        let idx = y * stride + x;

                        if idx < plane_data.len() {
                            sum += plane_data[idx] as u32;
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    let avg = sum / count;
                    let bit = avg > self.threshold as u32;

                    if bit {
                        code_bits |= 1 << bit_pos;
                    }

                    // Calculate confidence
                    let confidence = ((avg as i32 - self.threshold as i32).abs() as f32) / 128.0;
                    total_confidence += confidence.min(1.0);
                }

                bit_index += 1;
            }

            bch_codes[code_idx] = code_bits;
        }

        // Read CRC8
        let mut crc8_read = 0u8;
        for bit_pos in 0..8 {
            if bit_index >= max_bits {
                return None;
            }

            let block_x = (bit_index % max_blocks_x) * total_block_size;
            let block_y = (bit_index / max_blocks_x) * total_block_size;

            let sample_y = y_offset + block_y + block_size / 2;
            let sample_x = x_offset + block_x + block_size / 2;

            let mut sum = 0u32;
            let mut count = 0u32;

            for dy in 0..2.min(block_size / 2) {
                for dx in 0..2.min(block_size / 2) {
                    let y = sample_y + dy;
                    let x = sample_x + dx;
                    let idx = y * stride + x;

                    if idx < plane_data.len() {
                        sum += plane_data[idx] as u32;
                        count += 1;
                    }
                }
            }

            if count > 0 {
                let avg = sum / count;
                if avg > self.threshold as u32 {
                    crc8_read |= 1 << bit_pos;
                }
            }

            bit_index += 1;
        }

        // Check confidence
        let avg_confidence = total_confidence / 120.0; // 112 BCH bits + 8 CRC bits
        if avg_confidence < self.min_confidence {
            return None;
        }

        // First attempt: decode BCH codes with error correction
        let mut timestamp = 0u64;
        let mut corrected_count = 0;

        for i in 0..16 {
            let code = bch_codes[i];
            let original_nibble = (code >> 3) & 0xF;
            let corrected_nibble = decode_bch_7_4(code);

            if original_nibble != corrected_nibble {
                corrected_count += 1;
            }

            timestamp |= (corrected_nibble as u64) << ((15 - i) * 4);
        }

        // Validate with CRC8
        let calculated_crc = self.calculate_crc8(timestamp);

        // If CRC matches or we corrected errors, accept the result
        if calculated_crc == crc8_read {
            return Some(timestamp);
        }

        // If many corrections were made and CRC still fails, likely too corrupted
        if corrected_count > 4 {
            return None;
        }

        // Try accepting with some corrections even if CRC fails
        // (CRC itself might be corrupted)
        if corrected_count <= 2 && avg_confidence > 0.7 {
            return Some(timestamp);
        }

        None
    }

    fn calculate_crc8(&self, data: u64) -> u8 {
        let mut crc = 0u8;
        for i in 0..8 {
            let byte = ((data >> (i * 8)) & 0xFF) as u8;
            crc ^= byte;
            for _ in 0..8 {
                if crc & 0x80 != 0 {
                    crc = (crc << 1) ^ 0x07;
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }
}
