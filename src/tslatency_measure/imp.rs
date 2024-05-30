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
    VideoCapsBuilder, VideoFormat, VideoFrameRef,
};
use once_cell::sync::Lazy;
use std::sync::Mutex;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "tslatency_measure",
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
    x: u64,
    y: u64,
}

impl TsLatencyMeasure {
    fn parse_time_code(
        &self,
        frame: &mut VideoFrameRef<&mut BufferRef>,
        get_color: impl Fn(&[u8]) -> Option<bool>,
    ) {
        let Properties {
            x: start_x,
            y: start_y,
        } = *self.props.lock().unwrap();

        let curr_usecs = {
            let time = self.clock.time().unwrap();
            time.useconds()
        };

        let stride = frame.plane_stride()[0] as usize;
        let nb_channels = frame.format_info().pixel_stride()[0] as usize;
        let data = frame.plane_data_mut(0).unwrap();

        let start_x = start_x as usize;
        let start_y = start_y as usize;
        let end_x = start_x + 8;
        let end_y = start_y + 8;

        let mut bitmap = [0u8; 8];

        let lines = data[(start_y * stride)..(end_y * stride)].chunks_exact(stride);
        for (line, byte) in lines.zip(&mut bitmap) {
            let pixels =
                line[(start_x * nb_channels)..(end_x * nb_channels)].chunks_exact(nb_channels);

            *byte = pixels.enumerate().fold(0u8, |byte, (nth, pixel)| {
                if get_color(pixel).unwrap() {
                    byte | (1 << nth)
                } else {
                    byte
                }
            });
        }

        let prev_usecs = u64::from_be_bytes(bitmap);
        let diff_usecs = curr_usecs as i64 - prev_usecs as i64;

        info!(
            CAT,
            imp: self,
            "Delay {diff_usecs} usecs",
        );
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
        Self { x: 0, y: 0 }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TsLatencyMeasure {
    const NAME: &'static str = "GstTsLatencyMeasure";
    type Type = super::TsLatencyMeasure;
    type ParentType = gst::Element;
}

impl ObjectImpl for TsLatencyMeasure {
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
        match frame.format() {
            VideoFormat::Rgbx | VideoFormat::Rgb | VideoFormat::Bgrx | VideoFormat::Bgr => self
                .parse_time_code(frame, |p| {
                    Some(match p[0..3] {
                        [255, 255, 255] => true,
                        [0, 0, 0] => false,
                        _ => return None,
                    })
                }),
            VideoFormat::Rgba | VideoFormat::Bgra => self.parse_time_code(frame, |p| {
                Some(match p[0..4] {
                    [255, 255, 255, 255] => true,
                    [0, 0, 0, 255] => false,
                    _ => return None,
                })
            }),
            VideoFormat::Xrgb | VideoFormat::Xbgr => self.parse_time_code(frame, |p| {
                Some(match p[1..4] {
                    [255, 255, 255] => true,
                    [0, 0, 255] => false,
                    _ => return None,
                })
            }),
            VideoFormat::Argb | VideoFormat::Abgr => self.parse_time_code(frame, |p| {
                Some(match p[0..4] {
                    [255, 255, 255, 255] => true,
                    [255, 0, 0, 0] => false,
                    _ => return None,
                })
            }),
            _ => unimplemented!(),
        }

        Ok(FlowSuccess::Ok)
    }
}
