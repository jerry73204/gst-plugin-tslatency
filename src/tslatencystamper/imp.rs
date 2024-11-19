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
    VideoCapsBuilder, VideoFilter, VideoFormat, VideoFormatFlags, VideoFrameRef,
};
use itertools::{iproduct, izip};
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
        white_fill: &[u8],
        black_fill: &[u8],
    ) -> Result<(), FlowError> {
        let Properties {
            x: start_x,
            y: start_y,
            width,
            height,
        } = *self.props.lock().unwrap();

        // Get the current timestamp
        let usecs = self.clock.time().unwrap().useconds();
        let get_bit = |r: usize, c: usize| (usecs.to_be_bytes()[r] & (1 << c)) != 0;

        let fmt = frame.format_info();
        let row0 = start_y as usize;
        let rown = row0 + height as usize;
        let col0 = start_x as usize;
        let coln = col0 + width as usize;

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
            use VideoFormat::*;

            // src pad capabilities
            let caps = VideoCapsBuilder::new()
                .format_list([
                    Rgbx, Bgrx, Xrgb, Xbgr, Rgba, Bgra, Gbra, Argb, Abgr, Rgb, Bgr, Gbr, I420,
                    Yv12, Yvyu, Vyuy, Uyvy, Yuy2, Ayuv, Y41b, Y42b, Nv12, Nv16, Nv21, Nv24, Nv61,
                    A420, Yuv9, Yvu9, Iyu1,
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
        let fmt = frame.format_info();
        let flags = fmt.flags();

        if flags.contains(VideoFormatFlags::RGB) {
            self.stamp_time_code(frame, &[255, 255, 255], &[0, 0, 0])?;
        } else if flags.contains(VideoFormatFlags::YUV) {
            self.stamp_time_code(frame, &[255, 128, 128], &[0, 128, 128])?;
        } else {
            return Err(FlowError::NotSupported);
        }

        Ok(FlowSuccess::Ok)
    }
}
