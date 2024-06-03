use glib::subclass::{prelude::*, types::ObjectSubclass};
use gst::{
    error, info,
    subclass::{prelude::*, ElementMetadata},
    BufferRef, Clock, FlowError, FlowSuccess, PadDirection, PadPresence, PadTemplate, SystemClock,
};
use gst_base::subclass::BaseTransformMode;
use gst_video::{
    prelude::*,
    subclass::prelude::{BaseTransformImpl, VideoFilterImpl},
    VideoCapsBuilder, VideoFilter, VideoFormat, VideoFrameRef,
};
use itertools::izip;
use once_cell::sync::Lazy;
use rayon::prelude::*;
use std::sync::Mutex;

const DEFAULT_X: u32 = 0;
const DEFAULT_Y: u32 = 0;
const DEFAULT_WIDTH: u32 = 64;
const DEFAULT_HEIGHT: u32 = 64;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "tslatencymeasure",
        gst::DebugColorFlags::empty(),
        Some("Measure latency using binary time code stamped on frames"),
    )
});

pub struct TsLatencyMeasure {
    props: Mutex<Properties>,
    clock: Clock,
}

#[derive(Clone)]
struct Properties {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl TsLatencyMeasure {
    fn measure_latency_using_time_code(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
    ) -> Result<(), FlowError> {
        let Properties {
            x: start_x,
            y: start_y,
            width: crop_width,
            height: crop_height,
        } = *self.props.lock().unwrap();

        let curr_usecs = {
            let time = self.clock.time().unwrap();
            time.useconds()
        };

        let format = frame.format();
        let format_info = frame.format_info();
        let height_stride = frame.plane_stride()[0] as usize;
        let width_stride = format_info.pixel_stride()[0] as usize;
        let depth = format_info.depth()[0];
        let data = frame.plane_data(0).unwrap();

        debug_assert_eq!(format, VideoFormat::Rgba);
        if depth != 8 {
            error!(
                CAT,
                imp: self,
                "depth != 8 is not supported",
            );
            return Err(FlowError::NotSupported);
        }

        let bitmap_h = 8;
        let bitmap_w = 8;

        let col_range = {
            let start_x = start_x as usize;
            let end_x = start_x + crop_width as usize;
            (start_x * width_stride)..(end_x * width_stride)
        };
        let scale_x = bitmap_w as f32 / crop_width as f32;

        let row_range = {
            let start_y = start_y as usize;
            let end_y = start_y + crop_height as usize;
            (start_y * height_stride)..(end_y * height_stride)
        };
        let scale_y = bitmap_h as f32 / crop_height as f32;

        let scale = |x: usize, scale: f32| -> usize {
            (((x as f32 + 0.5) * scale - 0.5).round() + 0.5) as usize
        };
        let is_white = |rgba: &[u8]| {
            let &[r, g, b, _a] = rgba else {
                unreachable!();
            };
            ((r as f32 + g as f32 + b as f32) / 3.0).round() as u8 >= 128
        };

        let freq = data[row_range]
            .par_chunks_exact(height_stride)
            .enumerate()
            .flat_map(|(pr, line)| {
                let pixels = line[col_range.clone()].par_chunks_exact(width_stride);
                let br = scale(pr, scale_y).clamp(0, bitmap_h - 1);

                pixels.enumerate().map(move |(pc, pixel)| {
                    let bc = scale(pc, scale_x).clamp(0, bitmap_w - 1);
                    let color = if is_white(pixel) { 1 } else { 0 };
                    (br, bc, color)
                })
            })
            .fold(
                || [[[0, 0]; 8]; 8],
                |mut freq, (br, bc, color)| {
                    freq[br][bc][color] += 1;
                    freq
                },
            )
            .reduce_with(|mut lfreq, rfreq| {
                for (lrow, rrow) in izip!(&mut lfreq, rfreq) {
                    for (lcell, rcell) in izip!(lrow, rrow) {
                        let [lf0, lf1] = lcell;
                        let [rf0, rf1] = rcell;
                        *lf0 += rf0;
                        *lf1 += rf1;
                    }
                }
                lfreq
            })
            .unwrap();

        let mut bytes = [0u8; 8];

        freq.into_iter().zip(&mut bytes).for_each(|(row, byte)| {
            *byte = row
                .into_iter()
                .enumerate()
                .fold(0, |mut byte, (nth, [freq0, freq1])| {
                    let bit = freq1 > freq0;
                    if bit {
                        byte |= 1 << nth;
                    }
                    byte
                });
        });

        let stamped_usecs: u64 = u64::from_be_bytes(bytes);

        let diff_usecs = curr_usecs - stamped_usecs;
        info!(
            CAT,
            imp: self,
            "Delay {diff_usecs} usecs",
        );

        Ok(())
    }
}

impl Default for TsLatencyMeasure {
    fn default() -> Self {
        Self {
            props: Mutex::new(Properties::default()),
            clock: SystemClock::obtain(),
        }
    }
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            x: DEFAULT_X,
            y: DEFAULT_Y,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TsLatencyMeasure {
    const NAME: &'static str = "GstTsLatencyMeasure";
    type Type = super::TsLatencyMeasure;
    type ParentType = VideoFilter;
}

impl ObjectImpl for TsLatencyMeasure {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpecUInt64::builder("x")
                    .nick("x")
                    .blurb("Binary time code X position")
                    .default_value(0)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder("y")
                    .nick("y")
                    .blurb("Binary time code Y position")
                    .default_value(0)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "x" => {
                let mut props = self.props.lock().unwrap();
                let x = value.get().expect("type checked upstream");
                info!(
                    CAT,
                    imp: self,
                    "Changing x from {} to {}",
                    props.x,
                    x
                );
                props.x = x;
            }
            "y" => {
                let mut props = self.props.lock().unwrap();
                let y = value.get().expect("type checked upstream");
                info!(
                    CAT,
                    imp: self,
                    "Changing y from {} to {}",
                    props.y,
                    y
                );
                props.y = y;
            }
            "width" => {
                let mut props = self.props.lock().unwrap();
                let width = value.get().expect("type checked upstream");
                info!(
                    CAT,
                    imp: self,
                    "Changing width from {} to {}",
                    props.width,
                    width
                );
                props.width = width;
            }
            "height" => {
                let mut props = self.props.lock().unwrap();
                let height = value.get().expect("type checked upstream");
                info!(
                    CAT,
                    imp: self,
                    "Changing y from {} to {}",
                    props.height,
                    height
                );
                props.height = height;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "x" => {
                let props = self.props.lock().unwrap();
                props.x.to_value()
            }
            "y" => {
                let props = self.props.lock().unwrap();
                props.y.to_value()
            }
            "width" => {
                let props = self.props.lock().unwrap();
                props.width.to_value()
            }
            "height" => {
                let props = self.props.lock().unwrap();
                props.height.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for TsLatencyMeasure {}

impl ElementImpl for TsLatencyMeasure {
    fn metadata() -> Option<&'static ElementMetadata> {
        static ELEMENT_METADATA: Lazy<ElementMetadata> = Lazy::new(|| {
            ElementMetadata::new(
                "latency measurement using  binary time code",
                "Filter/Effect/Converter/Video",
                "Measure latency using binary time code stamped on frames",
                "Jerry Lin <jerry73204@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<PadTemplate>> = Lazy::new(|| {
            // src pad capabilities
            let caps = VideoCapsBuilder::new()
                .format_list([VideoFormat::Rgba])
                .build();

            let src_pad_template =
                PadTemplate::new("src", PadDirection::Src, PadPresence::Always, &caps).unwrap();

            let sink_pad_template =
                PadTemplate::new("sink", PadDirection::Sink, PadPresence::Always, &caps).unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl BaseTransformImpl for TsLatencyMeasure {
    const MODE: BaseTransformMode = BaseTransformMode::AlwaysInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;
}

impl VideoFilterImpl for TsLatencyMeasure {
    fn transform_frame_ip(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
    ) -> Result<FlowSuccess, FlowError> {
        self.measure_latency_using_time_code(frame)?;
        Ok(FlowSuccess::Ok)
    }
}
