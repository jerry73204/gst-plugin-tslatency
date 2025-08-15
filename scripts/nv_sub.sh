#!/usr/bin/env bash
set -e

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$script_dir/.."
cargo cbuild --release

cd "$script_dir"
export GST_PLUGIN_PATH="$PWD/../target/x86_64-unknown-linux-gnu/release:$GST_PLUGIN_PATH"
export GST_DEBUG=tslatencymeasure:4

gst-launch-1.0 -v \
               udpsrc port=5000 \
               ! h265parse \
               ! nvv4l2decoder \
               ! nvvideoconvert \
               ! tslatencymeasure \
               ! autovideosink
