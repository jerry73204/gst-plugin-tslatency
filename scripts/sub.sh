#!/usr/bin/env bash
set -e

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
cd "$script_dir"

export GST_PLUGIN_PATH="$PWD/../target/x86_64-unknown-linux-gnu/debug:$GST_PLUGIN_PATH"
export GST_DEBUG=tslatencymeasure:4

gst-launch-1.0 -v \
               udpsrc port=5000 \
               ! 'application/x-rtp,media=(string)video,clock-rate=(int)90000,encoding-name=(string)H264,payload=(int)96' \
               ! rtph264depay \
               ! decodebin \
               ! videoconvert \
               ! tslatencymeasure \
               ! videoconvert \
               ! autovideosink
