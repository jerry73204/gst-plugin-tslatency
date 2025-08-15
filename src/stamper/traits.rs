// Common traits and types for timestamp stampers

use gst_video::VideoFrameRef;
use gst::{BufferRef, FlowError, Clock};
use glib::prelude::*;

/// Stamper type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "GstTsLatencyStamperType")]
pub enum StamperType {
    /// Original implementation - simple binary encoding
    #[enum_value(name = "Original: Simple binary encoding", nick = "original")]
    Original,
    /// Optimized implementation - larger cells with error correction
    #[enum_value(name = "Optimized: Larger cells with CRC16", nick = "optimized")]
    Optimized,
    /// Fast robust implementation - BCH error correction
    #[enum_value(name = "Fast-Robust: BCH error correction", nick = "fast-robust")]
    FastRobust,
}

impl Default for StamperType {
    fn default() -> Self {
        StamperType::Optimized
    }
}

impl From<i32> for StamperType {
    fn from(value: i32) -> Self {
        match value {
            0 => StamperType::Original,
            1 => StamperType::Optimized,
            2 => StamperType::FastRobust,
            _ => StamperType::Optimized,
        }
    }
}

impl StamperType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StamperType::Original => "original",
            StamperType::Optimized => "optimized",
            StamperType::FastRobust => "fast-robust",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "original" => Some(StamperType::Original),
            "optimized" => Some(StamperType::Optimized),
            "fast-robust" | "fastrobust" => Some(StamperType::FastRobust),
            _ => None,
        }
    }
}

/// Common configuration for stampers
#[derive(Debug, Clone)]
pub struct StamperConfig {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Default for StamperConfig {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        }
    }
}

/// Common configuration for readers
#[derive(Debug, Clone)]
pub struct ReaderConfig {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub tolerance: u32,
}

impl Default for ReaderConfig {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
            tolerance: 5,
        }
    }
}

/// Trait for timestamp stamper implementations
pub trait TimestampStamper: Send + Sync {
    /// Stamp a timestamp onto a video frame
    fn stamp(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        clock: &Clock,
        config: &StamperConfig,
    ) -> Result<(), FlowError>;
    
    /// Get the name of this stamper implementation
    fn name(&self) -> &'static str;
    
    /// Get a description of this stamper
    fn description(&self) -> &'static str;
}

/// Trait for timestamp reader implementations
pub trait TimestampReader: Send + Sync {
    /// Read a timestamp from a video frame
    fn read(
        &self,
        frame: &VideoFrameRef<&BufferRef>,
        clock: &Clock,
        config: &ReaderConfig,
    ) -> Result<Option<u64>, FlowError>;
    
    /// Get the name of this reader implementation
    fn name(&self) -> &'static str;
    
    /// Get a description of this reader
    fn description(&self) -> &'static str;
}