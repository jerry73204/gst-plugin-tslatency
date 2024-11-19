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
    VideoCapsBuilder, VideoFilter, VideoFormat, VideoFormatFlags, VideoFrameRef,
};
use itertools::{iproduct, izip};
use once_cell::sync::Lazy;
use std::sync::Mutex;

const DEFAULT_X: u32 = 0;
const DEFAULT_Y: u32 = 0;
const DEFAULT_WIDTH: u32 = 64;
const DEFAULT_HEIGHT: u32 = 64;
const DEFAULT_TOLERANCE: u32 = 5;

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
    tolerance: u32,
}

impl TsLatencyMeasure {
    fn measure_latency_using_time_code(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        white_fill: &[u8],
        black_fill: &[u8],
    ) -> Result<(), FlowError> {
        let Properties {
            x: start_x,
            y: start_y,
            width: crop_width,
            height: crop_height,
            tolerance,
        } = *self.props.lock().unwrap();

        let curr_usecs = self.clock.time().unwrap().useconds();
        let fmt = frame.format_info();

        if fmt.bits() != 8 {
            error!(
                CAT,
                imp: self,
                "bits != 8 is not supported",
            );
            return Err(FlowError::NotSupported);
        }

        let row0 = start_y as usize;
        let rown = row0 + crop_height as usize;
        let col0 = start_x as usize;
        let coln = col0 + crop_width as usize;

        let abs_diff = |a: u8, b: u8| a.checked_sub(b).unwrap_or_else(|| b - a);
        let sub_scale = |val: usize, factor: u32| (-((-(val as i64)) >> factor)) as usize;

        // The white/black counts per bit in the 8x8 bitmap, indexed
        // by row, column and color code. Color code is 1 if white,
        // otherwise 0.
        let counts =
            iproduct!(row0..rown, col0..coln).fold([[[0; 2]; 8]; 8], |mut counts, (ir, ic)| {
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
            });
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
            tolerance: DEFAULT_TOLERANCE,
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
            "tolerance" => {
                let mut props = self.props.lock().unwrap();
                let tolerance = value.get().expect("type checked upstream");
                info!(
                    CAT,
                    imp: self,
                    "Changing tolerance from {} to {}",
                    props.tolerance,
                    tolerance
                );
                props.tolerance = tolerance;
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
            "tolerance" => {
                let props = self.props.lock().unwrap();
                props.tolerance.to_value()
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
        let fmt = frame.format_info();
        let flags = fmt.flags();

        if flags.contains(VideoFormatFlags::RGB) {
            self.measure_latency_using_time_code(frame, &[255, 255, 255], &[0, 0, 0])?;
        } else if flags.contains(VideoFormatFlags::YUV) {
            self.measure_latency_using_time_code(frame, &[255, 128, 128], &[0, 128, 128])?;
        } else {
            return Err(FlowError::NotSupported);
        }

        Ok(FlowSuccess::Ok)
    }
}
