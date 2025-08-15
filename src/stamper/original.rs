// Original timestamp stamper implementation
// This is the current implementation extracted from the existing code

use super::traits::{TimestampStamper, TimestampReader, StamperConfig, ReaderConfig};
use gst_video::{VideoFrameRef, VideoFormatFlags, prelude::*};
use gst::{BufferRef, Clock, FlowError, prelude::*};
use itertools::{iproduct, izip};

/// Original stamper implementation - simple 8x8 binary grid
pub struct OriginalStamper;

impl Default for OriginalStamper {
    fn default() -> Self {
        Self
    }
}

impl TimestampStamper for OriginalStamper {
    fn stamp(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        clock: &Clock,
        config: &StamperConfig,
    ) -> Result<(), FlowError> {
        let fmt = frame.format_info();
        let flags = fmt.flags();
        
        let (white_fill, black_fill) = if flags.contains(VideoFormatFlags::RGB) {
            ([255, 255, 255], [0, 0, 0])
        } else if flags.contains(VideoFormatFlags::YUV) {
            ([255, 128, 128], [0, 128, 128])
        } else {
            return Err(FlowError::NotSupported);
        };
        
        self.stamp_time_code(frame, clock, config, &white_fill, &black_fill)
    }
    
    fn name(&self) -> &'static str {
        "original"
    }
    
    fn description(&self) -> &'static str {
        "Original 8x8 binary timestamp encoder"
    }
}

impl OriginalStamper {
    fn stamp_time_code(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        clock: &Clock,
        config: &StamperConfig,
        white_fill: &[u8],
        black_fill: &[u8],
    ) -> Result<(), FlowError> {
        let start_x = config.x as usize;
        let start_y = config.y as usize;
        let width = config.width as usize;
        let height = config.height as usize;
        
        // Get the current timestamp
        let usecs = clock.time().unwrap().useconds();
        let get_bit = |r: usize, c: usize| (usecs.to_be_bytes()[r] & (1 << c)) != 0;
        
        let fmt = frame.format_info();
        let row0 = start_y;
        let rown = row0 + height;
        let col0 = start_x;
        let coln = col0 + width;
        
        let sub_scale = |val: usize, factor: u32| (-((-(val as i64)) >> factor)) as usize;
        
        for (ir, ic) in iproduct!(row0..rown, col0..coln) {
            let iter = izip!(
                fmt.plane(),
                fmt.pixel_stride(),
                fmt.poffset(),
                fmt.depth(),
                fmt.shift(),
                fmt.h_sub(),
                fmt.w_sub(),
                white_fill,
                black_fill
            );
            
            for args in iter {
                let (
                    &plane_ix,
                    &pixel_stride,
                    &poffset,
                    &depth,
                    &shift,
                    &h_sub,
                    &w_sub,
                    &white_val,
                    &black_val,
                ) = args;
                
                if depth != 8 || shift != 0 {
                    return Err(FlowError::NotSupported);
                }
                
                let plane_ix = plane_ix as usize;
                let plane_stride = frame.plane_stride()[plane_ix] as usize;
                let plane_data = frame.plane_data_mut(plane_ix as u32).unwrap();
                
                let pr = sub_scale(ir, h_sub);
                let pc = sub_scale(ic, w_sub);
                let offset = pr * plane_stride + pc * pixel_stride as usize + poffset as usize;
                
                if offset >= plane_data.len() {
                    continue;
                }
                
                let component = &mut plane_data[offset];
                
                let rr = ((ir - row0) as f32 + 0.5) / height as f32;
                let rc = ((ic - col0) as f32 + 0.5) / width as f32;
                let br = (rr * 8.0 - 0.5).round().clamp(0.0, 7.0) as usize;
                let bc = (rc * 8.0 - 0.5).round().clamp(0.0, 7.0) as usize;
                
                *component = if get_bit(br, bc) {
                    white_val
                } else {
                    black_val
                };
            }
        }
        
        Ok(())
    }
}

/// Original reader implementation
pub struct OriginalReader;

impl Default for OriginalReader {
    fn default() -> Self {
        Self
    }
}

impl TimestampReader for OriginalReader {
    fn read(
        &self,
        frame: &VideoFrameRef<&BufferRef>,
        clock: &Clock,
        config: &ReaderConfig,
    ) -> Result<Option<u64>, FlowError> {
        let fmt = frame.format_info();
        let flags = fmt.flags();
        
        let (white_fill, black_fill) = if flags.contains(VideoFormatFlags::RGB) {
            ([255, 255, 255], [0, 0, 0])
        } else if flags.contains(VideoFormatFlags::YUV) {
            ([255, 128, 128], [0, 128, 128])
        } else {
            return Err(FlowError::NotSupported);
        };
        
        self.measure_latency_using_time_code(frame, clock, config, &white_fill, &black_fill)
    }
    
    fn name(&self) -> &'static str {
        "original"
    }
    
    fn description(&self) -> &'static str {
        "Original 8x8 binary timestamp decoder with voting"
    }
}

impl OriginalReader {
    fn measure_latency_using_time_code(
        &self,
        frame: &VideoFrameRef<&BufferRef>,
        clock: &Clock,
        config: &ReaderConfig,
        white_fill: &[u8],
        black_fill: &[u8],
    ) -> Result<Option<u64>, FlowError> {
        let start_x = config.x as usize;
        let start_y = config.y as usize;
        let crop_width = config.width as usize;
        let crop_height = config.height as usize;
        let tolerance = config.tolerance;
        
        let fmt = frame.format_info();
        
        if fmt.bits() != 8 {
            return Err(FlowError::NotSupported);
        }
        
        let row0 = start_y;
        let rown = row0 + crop_height;
        let col0 = start_x;
        let coln = col0 + crop_width;
        
        let abs_diff = |a: u8, b: u8| a.checked_sub(b).unwrap_or_else(|| b - a);
        let sub_scale = |val: usize, factor: u32| (-((-(val as i64)) >> factor)) as usize;
        
        // The white/black counts per bit in the 8x8 bitmap
        let counts = iproduct!(row0..rown, col0..coln).fold(
            [[[0; 2]; 8]; 8],
            |mut counts, (ir, ic)| {
                let mut white_votes = 0;
                let mut black_votes = 0;
                
                for args in izip!(
                    fmt.plane(),
                    fmt.pixel_stride(),
                    fmt.poffset(),
                    fmt.depth(),
                    fmt.shift(),
                    fmt.h_sub(),
                    fmt.w_sub(),
                    white_fill,
                    black_fill
                ) {
                    let (
                        &plane_ix,
                        &pixel_stride,
                        &poffset,
                        _depth,
                        _shift,
                        &h_sub,
                        &w_sub,
                        &white_val,
                        &black_val,
                    ) = args;
                    
                    let plane_ix = plane_ix as usize;
                    let plane_stride = frame.plane_stride()[plane_ix] as usize;
                    let plane_data = frame.plane_data(plane_ix as u32).unwrap();
                    
                    let pr = sub_scale(ir, h_sub);
                    let pc = sub_scale(ic, w_sub);
                    let offset = pr * plane_stride + pc * pixel_stride as usize + poffset as usize;
                    
                    if offset >= plane_data.len() {
                        continue;
                    }
                    
                    let component = plane_data[offset];
                    
                    if (abs_diff(component, white_val) as u32) < tolerance {
                        white_votes += 1;
                    }
                    if (abs_diff(component, black_val) as u32) < tolerance {
                        black_votes += 1;
                    }
                }
                
                let rr = ((ir - row0) as f32 + 0.5) / crop_height as f32;
                let rc = ((ic - col0) as f32 + 0.5) / crop_width as f32;
                
                let br = (rr * 8.0 - 0.5).round().clamp(0.0, 7.0) as usize;
                let bc = (rc * 8.0 - 0.5).round().clamp(0.0, 7.0) as usize;
                
                if white_votes == fmt.n_components() {
                    counts[br][bc][1] += 1;
                }
                if black_votes == fmt.n_components() {
                    counts[br][bc][0] += 1;
                }
                
                counts
            },
        );
        
        let bytes = {
            let mut bytes = [0u8; 8];
            counts.into_iter().zip(&mut bytes).for_each(|(row, byte)| {
                *byte = row
                    .into_iter()
                    .enumerate()
                    .fold(0, |mut byte, (nth, [freq0, freq1])| {
                        if freq1 > freq0 {
                            byte |= 1 << nth;
                        }
                        byte
                    });
            });
            bytes
        };
        
        let stamped_usecs: u64 = u64::from_be_bytes(bytes);
        Ok(Some(stamped_usecs))
    }
}