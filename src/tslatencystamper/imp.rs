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
}

impl TsLatencyStamper {
    fn time_code_overlay(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        white: impl Fn(&mut [u8]),
        black: impl Fn(&mut [u8]),
    ) {
        let Properties {
            x: start_x,
            y: start_y,
        } = *self.props.lock().unwrap();

        let bitmap: [u8; 8] = {
            let time = self.clock.time().unwrap();
            let usecs = time.useconds();
            usecs.to_be_bytes()
        };

        let stride = frame.plane_stride()[0] as usize;
        let nb_channels = frame.format_info().pixel_stride()[0] as usize;
        let data = frame.plane_data_mut(0).unwrap();

        let start_x = start_x as usize;
        let start_y = start_y as usize;
        let end_x = start_x + 8;
        let end_y = start_y + 8;

        let lines = data[(start_y * stride)..(end_y * stride)].chunks_exact_mut(stride);

        for (line, byte) in lines.zip(bitmap) {
            let pixels =
                line[(start_x * nb_channels)..(end_x * nb_channels)].chunks_exact_mut(nb_channels);

            for (nth, pixel) in pixels.enumerate() {
                let color: bool = byte & (1 << nth) != 0;
                if color {
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
        Self { x: 0, y: 0 }
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
                    "Changing hue-shift from {} to {}",
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
                    "Changing hue-shift from {} to {}",
                    props.y,
                    y
                );
                props.y = y;
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
                .time_code_overlay(
                    frame,
                    |p| {
                        p[0..3].fill(255);
                    },
                    |p| {
                        p[0..3].fill(0);
                    },
                ),
            VideoFormat::Rgba | VideoFormat::Bgra => self.time_code_overlay(
                frame,
                |p| {
                    p[0..4].fill(255);
                },
                |p| {
                    p[0..4].copy_from_slice(&[0, 0, 0, 255]);
                },
            ),
            VideoFormat::Xrgb | VideoFormat::Xbgr => self.time_code_overlay(
                frame,
                |p| {
                    p[1..4].fill(255);
                },
                |p| {
                    p[1..4].fill(0);
                },
            ),
            VideoFormat::Argb | VideoFormat::Abgr => self.time_code_overlay(
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
