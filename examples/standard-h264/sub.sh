#!/usr/bin/env bash
set -e

# Standard H.264 Subscriber using avdec_h264 or openh264dec

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
project_root="$script_dir/../.."

# Build the plugin
cd "$project_root"
cargo cbuild --release

# Set up plugin path
export GST_PLUGIN_PATH="$project_root/target/x86_64-unknown-linux-gnu/release:$GST_PLUGIN_PATH"

# Configuration
LISTEN_PORT="${1:-5000}"
STAMPER_TYPE="${2:-optimized}"  # Must match publisher!

echo "========================================================"
echo "Standard H.264 Subscriber (Software Decoder)"
echo "========================================================"
echo "Listen Port: $LISTEN_PORT"
echo "Stamper Type: $STAMPER_TYPE (must match publisher)"
echo ""
echo "Usage: $0 [port] [stamper-type]"
echo "Example: $0 5000 fast-robust"
echo "========================================================"

# Detect available decoder
if gst-inspect-1.0 avdec_h264 &>/dev/null; then
    DECODER="avdec_h264"
    echo "Decoder: avdec_h264"
elif gst-inspect-1.0 openh264dec &>/dev/null; then
    DECODER="openh264dec"
    echo "Decoder: openh264dec"
else
    echo "ERROR: No H.264 decoder found!"
    echo "Install with: sudo apt install gstreamer1.0-libav"
    exit 1
fi

echo "========================================================"
echo "Waiting for stream on port $LISTEN_PORT..."
echo ""

# Enable latency measurement debug
export GST_DEBUG=tslatencymeasure:4

gst-launch-1.0 -v \
    udpsrc port=$LISTEN_PORT caps="application/x-rtp,media=video,clock-rate=90000,encoding-name=H264,payload=96" \
    ! rtph264depay \
    ! h264parse \
    ! $DECODER \
    ! tslatencymeasure stamper-type=$STAMPER_TYPE \
    ! videoconvert \
    ! autovideosink sync=false