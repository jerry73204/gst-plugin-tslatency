mod imp;

use gst::prelude::*;

glib::wrapper! {
    pub struct TsLatencyMeasure(ObjectSubclass<imp::TsLatencyMeasure>) @extends gst_base::BaseTransform, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "tslatency_measure",
        gst::Rank::PRIMARY,
        TsLatencyMeasure::static_type(),
    )
}
