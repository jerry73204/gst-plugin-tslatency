#!/usr/bin/env bash
set -e

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$script_dir/.."
cargo cbuild --release

cd "$script_dir"
export GST_PLUGIN_PATH="$PWD/../target/x86_64-unknown-linux-gnu/release:$GST_PLUGIN_PATH"
export GST_DEBUG=tslatencystamper:4,tslatencymeasure:4

gst-launch-1.0 -v \
               videotestsrc \
               ! 'video/x-raw,width=1920,height=1080,format=(string)I420' \
               ! tslatencystamper \
               ! tslatencymeasure \
               ! autovideosink

