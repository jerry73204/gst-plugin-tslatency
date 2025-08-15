use crate::stamper::{create_reader, ReaderConfig, StamperType, TimestampReader};
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
    reader: Mutex<Box<dyn TimestampReader>>,
}

#[derive(Clone)]
struct Properties {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    tolerance: u32,
    stamper_type: StamperType,
}

impl Default for TsLatencyMeasure {
    fn default() -> Self {
        let stamper_type = StamperType::default();
        Self {
            props: Mutex::new(Properties::default()),
            clock: SystemClock::obtain(),
            reader: Mutex::new(create_reader(stamper_type)),
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
            stamper_type: StamperType::default(),
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
                glib::ParamSpecUInt64::builder("width")
                    .nick("width")
                    .blurb("Binary time code width")
                    .default_value(DEFAULT_WIDTH as u64)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder("height")
                    .nick("height")
                    .blurb("Binary time code height")
                    .default_value(DEFAULT_HEIGHT as u64)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("tolerance")
                    .nick("tolerance")
                    .blurb("Tolerance for color matching")
                    .default_value(DEFAULT_TOLERANCE)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecEnum::builder::<StamperType>("stamper-type")
                    .nick("Stamper Type")
                    .blurb("Type of timestamp reader to use (must match stamper)")
                    .default_value(StamperType::default())
                    .mutable_ready()
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
            "stamper-type" => {
                let mut props = self.props.lock().unwrap();
                let stamper_type = value.get().expect("type checked upstream");
                info!(
                    CAT,
                    imp: self,
                    "Changing stamper type to {:?}",
                    stamper_type
                );
                props.stamper_type = stamper_type;
                *self.reader.lock().unwrap() = create_reader(stamper_type);
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
            "stamper-type" => {
                let props = self.props.lock().unwrap();
                props.stamper_type.to_value()
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
        let props = self.props.lock().unwrap();
        let config = ReaderConfig {
            x: props.x,
            y: props.y,
            width: props.width,
            height: props.height,
            tolerance: props.tolerance,
        };
        drop(props);

        let reader = self.reader.lock().unwrap();
        match reader.read(frame, &self.clock, &config)? {
            Some(stamped_usecs) => {
                let curr_usecs = self.clock.time().unwrap().useconds();
                let diff_usecs = curr_usecs - stamped_usecs;
                info!(
                    CAT,
                    imp: self,
                    "Delay {} usecs",
                    diff_usecs
                );
            }
            None => {
                error!(
                    CAT,
                    imp: self,
                    "Failed to read timestamp from frame"
                );
            }
        }

        Ok(FlowSuccess::Ok)
    }
}
