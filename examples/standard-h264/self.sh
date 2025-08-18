#!/usr/bin/env bash
set -e

# Self-contained test using standard H.264 software codecs
# Tests encoding and decoding in a single pipeline

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
project_root="$script_dir/../.."

# Build the plugin
cd "$project_root"
cargo cbuild --release

# Set up plugin path
export GST_PLUGIN_PATH="$project_root/target/x86_64-unknown-linux-gnu/release:$GST_PLUGIN_PATH"
export GST_DEBUG=tslatencystamper:4,tslatencymeasure:4

# Configuration
STAMPER_TYPE="${1:-optimized}"
DURATION="${2:-100}"  # Number of frames

echo "========================================================"
echo "Standard H.264 Self-Test (Single Pipeline)"
echo "========================================================"
echo "Stamper Type: $STAMPER_TYPE"
echo "Duration: $DURATION frames"
echo ""
echo "This tests encoding → decoding latency in one pipeline"
echo "Usage: $0 [stamper-type] [num-frames]"
echo "========================================================"

# Check for codecs
if ! gst-inspect-1.0 x264enc &>/dev/null; then
    echo "ERROR: x264enc not found!"
    echo "Install with: sudo apt install gstreamer1.0-plugins-ugly"
    exit 1
fi

if gst-inspect-1.0 avdec_h264 &>/dev/null; then
    H264_DECODER="avdec_h264"
    echo "Decoder: avdec_h264"
elif gst-inspect-1.0 openh264dec &>/dev/null; then
    H264_DECODER="openh264dec"
    echo "Decoder: openh264dec"
else
    echo "ERROR: No H.264 decoder found!"
    echo "Install with: sudo apt install gstreamer1.0-libav"
    exit 1
fi

echo "========================================================"
echo "Running test..."
echo ""

# Run self-contained pipeline
gst-launch-1.0 -v \
    videotestsrc pattern=smpte num-buffers=$DURATION \
    ! 'video/x-raw,width=1920,height=1080,format=I420,framerate=30/1' \
    ! tslatencystamper stamper-type=$STAMPER_TYPE \
    ! x264enc tune=zerolatency bitrate=4000 key-int-max=30 \
    ! h264parse \
    ! $H264_DECODER \
    ! tslatencymeasure stamper-type=$STAMPER_TYPE \
    ! videoconvert \
    ! autovideosink sync=false

echo ""
echo "Test completed!"