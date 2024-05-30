mod tslatency_measure;
mod tslatency_stamper;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    tslatency_stamper::register(plugin)?;
    tslatency_measure::register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    tslatency,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/X11",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
