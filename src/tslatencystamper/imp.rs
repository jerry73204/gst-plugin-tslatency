use glib::subclass::{prelude::*, types::ObjectSubclass};
use gst::{
    info,
    subclass::{prelude::*, ElementMetadata},
    BufferRef, Clock, FlowError, FlowSuccess, PadDirection, PadPresence, PadTemplate, SystemClock,
};
use gst_base::subclass::BaseTransformMode;
use gst_video::{
    prelude::*,
    subclass::prelude::{BaseTransformImpl, VideoFilterImpl},
    VideoCapsBuilder, VideoFilter, VideoFormat, VideoFrameRef,
};
use once_cell::sync::Lazy;
use std::sync::Mutex;

const DEFAULT_X: u64 = 0;
const DEFAULT_Y: u64 = 0;
const DEFAULT_WIDTH: u64 = 64;
const DEFAULT_HEIGHT: u64 = 64;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "tslatencystamper",
        gst::DebugColorFlags::empty(),
        Some("Binary time code stamper"),
    )
});

pub struct TsLatencyStamper {
    props: Mutex<Properties>,
    clock: Clock,
}

#[derive(Clone)]
struct Properties {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

impl TsLatencyStamper {
    fn stamp_time_code(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        white: impl Fn(&mut [u8]),
        black: impl Fn(&mut [u8]),
    ) {
        let Properties {
            x: start_x,
            y: start_y,
            width,
            height,
        } = *self.props.lock().unwrap();

        let usecs = self.clock.time().unwrap().useconds();

        let bitmap = {
            let mut bitmap = [[false; 8]; 8];
            let bytes = usecs.to_be_bytes();
            bytes.into_iter().zip(&mut bitmap).for_each(|(byte, row)| {
                (0..u8::BITS)
                    .map(|nth| 1 << nth)
                    .map(|bit| (byte & bit) != 0)
                    .zip(row)
                    .for_each(|(bit, cell)| {
                        *cell = bit;
                    });
            });

            bitmap
        };

        let bitmap_h = bitmap.len();
        let bitmap_w = bitmap[0].len();

        let height_stride = frame.plane_stride()[0] as usize;
        let width_stride = frame.format_info().pixel_stride()[0] as usize;
        let data = frame.plane_data_mut(0).unwrap();

        let start_x = start_x as usize;
        let end_x = start_x + width as usize;
        let col_range = (start_x * width_stride)..(end_x * width_stride);
        let scale_x = bitmap_w as f32 / width as f32;

        let start_y = start_y as usize;
        let end_y = start_y + height as usize;
        let row_range = (start_y * height_stride)..(end_y * height_stride);
        let scale_y = bitmap_h as f32 / height as f32;

        let lines = data[row_range].chunks_exact_mut(height_stride);

        let scale = |x: usize, scale: f32| -> usize {
            (((x as f32 + 0.5) * scale - 0.5).round() + 0.5) as usize
        };

        for (pr, line) in lines.enumerate() {
            let pixels = line[col_range.clone()].chunks_exact_mut(width_stride);
            let br = scale(pr, scale_y).clamp(0, bitmap_h - 1);
            let bit_row = &bitmap[br];

            for (pc, pixel) in pixels.enumerate() {
                let bc = scale(pc, scale_x).clamp(0, bitmap_w - 1);
                let bit = bit_row[bc];

                if bit {
                    white(pixel);
                } else {
                    black(pixel);
                }
            }
        }
    }
}

impl Default for TsLatencyStamper {
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
impl ObjectSubclass for TsLatencyStamper {
    const NAME: &'static str = "GstTsLatencyStamper";
    type Type = super::TsLatencyStamper;
    type ParentType = VideoFilter;
}

impl ObjectImpl for TsLatencyStamper {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpecUInt64::builder("x")
                    .nick("x")
                    .blurb("Time code X position")
                    .default_value(DEFAULT_X)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder("y")
                    .nick("y")
                    .blurb("Time code Y position")
                    .default_value(DEFAULT_Y)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder("width")
                    .nick("w")
                    .blurb("Time code width")
                    .default_value(DEFAULT_WIDTH)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder("height")
                    .nick("h")
                    .blurb("Time code height")
                    .default_value(DEFAULT_HEIGHT)
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
                    "Changing height from {} to {}",
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

impl GstObjectImpl for TsLatencyStamper {}

impl ElementImpl for TsLatencyStamper {
    fn metadata() -> Option<&'static ElementMetadata> {
        static ELEMENT_METADATA: Lazy<ElementMetadata> = Lazy::new(|| {
            ElementMetadata::new(
                "Binary time code stamper",
                "Filter/Effect/Converter/Video",
                "Stamp binary time code overlay on incoming frames",
                "Jerry Lin <jerry73204@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<PadTemplate>> = Lazy::new(|| {
            // src pad capabilities
            let caps = VideoCapsBuilder::new()
                .format_list([
                    VideoFormat::Rgbx,
                    VideoFormat::Xrgb,
                    VideoFormat::Bgrx,
                    VideoFormat::Xbgr,
                    VideoFormat::Rgba,
                    VideoFormat::Argb,
                    VideoFormat::Bgra,
                    VideoFormat::Abgr,
                    VideoFormat::Rgb,
                    VideoFormat::Bgr,
                ])
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

impl BaseTransformImpl for TsLatencyStamper {
    const MODE: BaseTransformMode = BaseTransformMode::AlwaysInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;
}

impl VideoFilterImpl for TsLatencyStamper {
    fn transform_frame_ip(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
    ) -> Result<FlowSuccess, FlowError> {
        match frame.format() {
            VideoFormat::Rgbx | VideoFormat::Rgb | VideoFormat::Bgrx | VideoFormat::Bgr => self
                .stamp_time_code(
                    frame,
                    |p| {
                        p[0..3].fill(255);
                    },
                    |p| {
                        p[0..3].fill(0);
                    },
                ),
            VideoFormat::Rgba | VideoFormat::Bgra => self.stamp_time_code(
                frame,
                |p| {
                    p[0..4].fill(255);
                },
                |p| {
                    p[0..4].copy_from_slice(&[0, 0, 0, 255]);
                },
            ),
            VideoFormat::Xrgb | VideoFormat::Xbgr => self.stamp_time_code(
                frame,
                |p| {
                    p[1..4].fill(255);
                },
                |p| {
                    p[1..4].fill(0);
                },
            ),
            VideoFormat::Argb | VideoFormat::Abgr => self.stamp_time_code(
                frame,
                |p| {
                    p[0..4].fill(255);
                },
                |p| {
                    p[0..4].copy_from_slice(&[255, 0, 0, 0]);
                },
            ),
            _ => unimplemented!(),
        }
        Ok(FlowSuccess::Ok)
    }
}
