// Optimized timestamp stamper implementation with error correction

use super::traits::{TimestampStamper, TimestampReader, StamperConfig, ReaderConfig};
use gst_video::{VideoFrameRef, VideoFormatFlags, VideoFormat, prelude::*};
use gst::{BufferRef, Clock, FlowError, prelude::*};

/// Optimized stamper with larger cells and error correction
pub struct OptimizedStamper {
    cell_size: usize,
    grid_width: usize,
    grid_height: usize,
    start_marker: u16,
    end_marker: u16,
}

impl Default for OptimizedStamper {
    fn default() -> Self {
        Self {
            cell_size: 8,      // 8x8 pixels per bit
            grid_width: 12,    // 12 cells wide
            grid_height: 8,    // 8 cells high  
            start_marker: 0xA5A5,  // Start pattern
            end_marker: 0x5A5A,    // End pattern
        }
    }
}

impl TimestampStamper for OptimizedStamper {
    fn stamp(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        clock: &Clock,
        config: &StamperConfig,
    ) -> Result<(), FlowError> {
        let timestamp_usecs = clock.time().unwrap().useconds();
        let encoded = self.encode_with_redundancy(timestamp_usecs);
        
        let format = frame.format();
        
        if format == VideoFormat::I420 {
            self.stamp_i420_fast(frame, &encoded, config)?;
        } else {
            self.stamp_generic(frame, &encoded, config)?;
        }
        
        Ok(())
    }
    
    fn name(&self) -> &'static str {
        "optimized"
    }
    
    fn description(&self) -> &'static str {
        "Optimized stamper with 8x8 cells and CRC16 error detection"
    }
}

impl OptimizedStamper {
    fn encode_with_redundancy(&self, timestamp_usecs: u64) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(16);
        
        // Add start marker (2 bytes)
        encoded.push((self.start_marker >> 8) as u8);
        encoded.push(self.start_marker as u8);
        
        // Encode 48-bit timestamp (6 bytes) - enough for ~8 years
        let ts48 = timestamp_usecs & 0xFFFF_FFFF_FFFF;
        encoded.push((ts48 >> 40) as u8);
        encoded.push((ts48 >> 32) as u8);
        encoded.push((ts48 >> 24) as u8);
        encoded.push((ts48 >> 16) as u8);
        encoded.push((ts48 >> 8) as u8);
        encoded.push(ts48 as u8);
        
        // Add CRC16 checksum (2 bytes)
        let crc = self.crc16(&encoded[2..8]);
        encoded.push((crc >> 8) as u8);
        encoded.push(crc as u8);
        
        // Add end marker (2 bytes)
        encoded.push((self.end_marker >> 8) as u8);
        encoded.push(self.end_marker as u8);
        
        encoded
    }
    
    fn stamp_i420_fast(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        encoded: &[u8],
        config: &StamperConfig,
    ) -> Result<(), FlowError> {
        let stride = frame.plane_stride()[0] as usize;
        let plane_data = frame.plane_data_mut(0).unwrap();
        
        // Use gray levels that survive compression better
        const WHITE: u8 = 235;  // Not pure white
        const BLACK: u8 = 20;   // Not pure black
        const GRAY: u8 = 128;   // Mid-gray for transitions
        
        let x_offset = config.x as usize;
        let y_offset = config.y as usize;
        
        let mut bit_index = 0;
        
        for byte in encoded {
            for bit_pos in 0..8 {
                let bit = (byte >> (7 - bit_pos)) & 1 == 1;
                
                // Calculate cell position
                let cell_x = bit_index % self.grid_width;
                let cell_y = bit_index / self.grid_width;
                
                if cell_y >= self.grid_height {
                    break;
                }
                
                let x_start = x_offset + cell_x * self.cell_size;
                let y_start = y_offset + cell_y * self.cell_size;
                
                // Stamp with gradient edges for better compression survival
                self.stamp_cell_with_gradient(
                    plane_data,
                    stride,
                    x_start,
                    y_start,
                    if bit { WHITE } else { BLACK },
                    GRAY,
                );
                
                bit_index += 1;
            }
        }
        
        Ok(())
    }
    
    fn stamp_cell_with_gradient(
        &self,
        data: &mut [u8],
        stride: usize,
        x: usize,
        y: usize,
        center_value: u8,
        edge_value: u8,
    ) {
        let size = self.cell_size;
        let edge_width = 1; // 1-pixel gradient edge
        
        for dy in 0..size {
            for dx in 0..size {
                let idx = (y + dy) * stride + (x + dx);
                
                if idx >= data.len() {
                    continue;
                }
                
                // Apply gradient at edges
                let value = if dy < edge_width || dy >= size - edge_width ||
                              dx < edge_width || dx >= size - edge_width {
                    // Blend edge with center
                    ((center_value as u16 + edge_value as u16) / 2) as u8
                } else {
                    center_value
                };
                
                data[idx] = value;
            }
        }
    }
    
    fn stamp_generic(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        encoded: &[u8],
        config: &StamperConfig,
    ) -> Result<(), FlowError> {
        let fmt = frame.format_info();
        let flags = fmt.flags();
        
        let (white, black) = if flags.contains(VideoFormatFlags::RGB) {
            ([235, 235, 235], [20, 20, 20])
        } else if flags.contains(VideoFormatFlags::YUV) {
            ([235, 128, 128], [20, 128, 128])
        } else {
            return Err(FlowError::NotSupported);
        };
        
        // Simplified generic implementation
        // In production, this would handle multiple planes properly
        let stride = frame.plane_stride()[0] as usize;
        let plane_data = frame.plane_data_mut(0).unwrap();
        
        let x_offset = config.x as usize;
        let y_offset = config.y as usize;
        
        let mut bit_index = 0;
        
        for byte in encoded {
            for bit_pos in 0..8 {
                let bit = (byte >> (7 - bit_pos)) & 1 == 1;
                let cell_x = bit_index % self.grid_width;
                let cell_y = bit_index / self.grid_width;
                
                if cell_y >= self.grid_height {
                    break;
                }
                
                let x_start = x_offset + cell_x * self.cell_size;
                let y_start = y_offset + cell_y * self.cell_size;
                
                self.stamp_cell_with_gradient(
                    plane_data,
                    stride,
                    x_start,
                    y_start,
                    if bit { white[0] } else { black[0] },
                    128,
                );
                
                bit_index += 1;
            }
        }
        
        Ok(())
    }
    
    fn crc16(&self, data: &[u8]) -> u16 {
        let mut crc = 0xFFFF_u16;
        
        for &byte in data {
            crc ^= (byte as u16) << 8;
            for _ in 0..8 {
                if crc & 0x8000 != 0 {
                    crc = (crc << 1) ^ 0x1021;
                } else {
                    crc <<= 1;
                }
            }
        }
        
        crc
    }
}

/// Optimized reader with error detection
pub struct OptimizedReader {
    cell_size: usize,
    grid_width: usize,
    grid_height: usize,
    start_marker: u16,
    end_marker: u16,
    threshold: u8,
    min_confidence: f32,
}

impl Default for OptimizedReader {
    fn default() -> Self {
        Self {
            cell_size: 8,
            grid_width: 12,
            grid_height: 8,
            start_marker: 0xA5A5,
            end_marker: 0x5A5A,
            threshold: 128,
            min_confidence: 0.6,
        }
    }
}

impl TimestampReader for OptimizedReader {
    fn read(
        &self,
        frame: &VideoFrameRef<&BufferRef>,
        _clock: &Clock,
        config: &ReaderConfig,
    ) -> Result<Option<u64>, FlowError> {
        let format = frame.format();
        
        let decoded = if format == VideoFormat::I420 {
            self.read_i420_fast(frame, config)?
        } else {
            self.read_generic(frame, config)?
        };
        
        // Verify and extract timestamp
        Ok(self.verify_and_extract(&decoded))
    }
    
    fn name(&self) -> &'static str {
        "optimized"
    }
    
    fn description(&self) -> &'static str {
        "Optimized reader with CRC16 validation and confidence thresholds"
    }
}

impl OptimizedReader {
    fn read_i420_fast(
        &self,
        frame: &VideoFrameRef<&BufferRef>,
        config: &ReaderConfig,
    ) -> Result<Vec<u8>, FlowError> {
        let stride = frame.plane_stride()[0] as usize;
        let plane_data = frame.plane_data(0).unwrap();
        
        let x_offset = config.x as usize;
        let y_offset = config.y as usize;
        
        let mut decoded = Vec::with_capacity(12);
        let mut bit_buffer = 0u8;
        let mut bit_count = 0;
        
        for cell_y in 0..self.grid_height {
            for cell_x in 0..self.grid_width {
                let x_start = x_offset + cell_x * self.cell_size;
                let y_start = y_offset + cell_y * self.cell_size;
                
                // Read cell with majority voting
                let bit = self.read_cell_majority(
                    plane_data,
                    stride,
                    x_start,
                    y_start,
                ).unwrap_or(false);
                
                bit_buffer = (bit_buffer << 1) | (bit as u8);
                bit_count += 1;
                
                if bit_count == 8 {
                    decoded.push(bit_buffer);
                    bit_buffer = 0;
                    bit_count = 0;
                    
                    if decoded.len() >= 12 {
                        return Ok(decoded);
                    }
                }
            }
        }
        
        Ok(decoded)
    }
    
    fn read_cell_majority(
        &self,
        data: &[u8],
        stride: usize,
        x: usize,
        y: usize,
    ) -> Option<bool> {
        let size = self.cell_size;
        
        let mut sum = 0u32;
        let mut count = 0u32;
        
        // Sample inner region (avoiding edges affected by compression)
        for dy in 1..size - 1 {
            for dx in 1..size - 1 {
                let idx = (y + dy) * stride + (x + dx);
                
                if idx < data.len() {
                    sum += data[idx] as u32;
                    count += 1;
                }
            }
        }
        
        if count == 0 {
            return None;
        }
        
        let avg = sum / count;
        
        // Calculate confidence
        let distance_from_threshold = ((avg as i32 - self.threshold as i32).abs() as f32) / 128.0;
        
        if distance_from_threshold < (1.0 - self.min_confidence) {
            return None; // Too close to threshold, unreliable
        }
        
        Some(avg > self.threshold as u32)
    }
    
    fn read_generic(
        &self,
        frame: &VideoFrameRef<&BufferRef>,
        config: &ReaderConfig,
    ) -> Result<Vec<u8>, FlowError> {
        // Simplified - just read from first plane
        self.read_i420_fast(frame, config)
    }
    
    fn verify_and_extract(&self, data: &[u8]) -> Option<u64> {
        if data.len() < 12 {
            return None;
        }
        
        // Check start marker
        let start = ((data[0] as u16) << 8) | (data[1] as u16);
        if start != self.start_marker {
            return None;
        }
        
        // Check end marker
        let end = ((data[10] as u16) << 8) | (data[11] as u16);
        if end != self.end_marker {
            return None;
        }
        
        // Verify CRC
        let stored_crc = ((data[8] as u16) << 8) | (data[9] as u16);
        let calculated_crc = self.crc16(&data[2..8]);
        
        if stored_crc != calculated_crc {
            return None;
        }
        
        // Extract timestamp
        let timestamp = ((data[2] as u64) << 40) |
                       ((data[3] as u64) << 32) |
                       ((data[4] as u64) << 24) |
                       ((data[5] as u64) << 16) |
                       ((data[6] as u64) << 8) |
                       (data[7] as u64);
        
        Some(timestamp)
    }
    
    fn crc16(&self, data: &[u8]) -> u16 {
        let mut crc = 0xFFFF_u16;
        
        for &byte in data {
            crc ^= (byte as u16) << 8;
            for _ in 0..8 {
                if crc & 0x8000 != 0 {
                    crc = (crc << 1) ^ 0x1021;
                } else {
                    crc <<= 1;
                }
            }
        }
        
        crc
    }
}