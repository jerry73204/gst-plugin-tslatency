use crate::stamper::{create_stamper, StamperConfig, StamperType, TimestampStamper};
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
    stamper: Mutex<Box<dyn TimestampStamper>>,
}

#[derive(Clone)]
struct Properties {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
    stamper_type: StamperType,
}

impl Default for TsLatencyStamper {
    fn default() -> Self {
        let stamper_type = StamperType::default();
        Self {
            props: Mutex::new(Properties::default()),
            clock: SystemClock::obtain(),
            stamper: Mutex::new(create_stamper(stamper_type)),
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
            stamper_type: StamperType::default(),
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
                glib::ParamSpecEnum::builder::<StamperType>("stamper-type")
                    .nick("Stamper Type")
                    .blurb("Type of timestamp stamper to use")
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
                    "Changing height from {} to {}",
                    props.height,
                    height
                );
                props.height = height;
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
                *self.stamper.lock().unwrap() = create_stamper(stamper_type);
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
            "stamper-type" => {
                let props = self.props.lock().unwrap();
                props.stamper_type.to_value()
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
        let props = self.props.lock().unwrap();
        let config = StamperConfig {
            x: props.x as u32,
            y: props.y as u32,
            width: props.width as u32,
            height: props.height as u32,
        };
        drop(props);

        let stamper = self.stamper.lock().unwrap();
        stamper.stamp(frame, &self.clock, &config)?;

        Ok(FlowSuccess::Ok)
    }
}
