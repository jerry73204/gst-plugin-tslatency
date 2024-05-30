mod imp;

use gst::prelude::*;

glib::wrapper! {
    pub struct TsLatencyStamper(ObjectSubclass<imp::TsLatencyStamper>) @extends gst_base::BaseTransform, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "tslatencystamper",
        gst::Rank::NONE,
        TsLatencyStamper::static_type(),
    )
}
