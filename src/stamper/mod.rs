// Timestamp stamper module with multiple implementation strategies

pub mod original;
pub mod optimized;
pub mod fast_robust;
pub mod traits;

pub use traits::{TimestampStamper, TimestampReader, StamperType, StamperConfig, ReaderConfig};
pub use original::{OriginalStamper, OriginalReader};
pub use optimized::{OptimizedStamper, OptimizedReader};
pub use fast_robust::{FastRobustStamper, FastRobustReader};

use gst_video::VideoFormatFlags;
use gst::FlowError;

/// Factory function to create a stamper based on the selected type
pub fn create_stamper(stamper_type: StamperType) -> Box<dyn TimestampStamper> {
    match stamper_type {
        StamperType::Original => Box::new(OriginalStamper::default()),
        StamperType::Optimized => Box::new(OptimizedStamper::default()),
        StamperType::FastRobust => Box::new(FastRobustStamper::default()),
    }
}

/// Factory function to create a reader based on the selected type
pub fn create_reader(stamper_type: StamperType) -> Box<dyn TimestampReader> {
    match stamper_type {
        StamperType::Original => Box::new(OriginalReader::default()),
        StamperType::Optimized => Box::new(OptimizedReader::default()),
        StamperType::FastRobust => Box::new(FastRobustReader::default()),
    }
}

/// Helper function to get appropriate fill values for different video formats
pub fn get_fill_values(flags: VideoFormatFlags) -> Result<([u8; 3], [u8; 3]), FlowError> {
    if flags.contains(VideoFormatFlags::RGB) {
        Ok(([255, 255, 255], [0, 0, 0]))
    } else if flags.contains(VideoFormatFlags::YUV) {
        Ok(([255, 128, 128], [0, 128, 128]))
    } else {
        Err(FlowError::NotSupported)
    }
}

/// Helper function to get robust fill values (not pure black/white)
pub fn get_robust_fill_values(flags: VideoFormatFlags) -> Result<([u8; 3], [u8; 3]), FlowError> {
    if flags.contains(VideoFormatFlags::RGB) {
        Ok(([235, 235, 235], [20, 20, 20]))
    } else if flags.contains(VideoFormatFlags::YUV) {
        Ok(([235, 128, 128], [20, 128, 128]))
    } else {
        Err(FlowError::NotSupported)
    }
}