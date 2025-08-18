#!/usr/bin/env bash
set -e

# NVIDIA Hardware-Accelerated H.264 Subscriber using NVDEC

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
project_root="$script_dir/../.."

# Build the plugin
cd "$project_root"
cargo cbuild --release

# Set up plugin path
export GST_PLUGIN_PATH="$project_root/target/x86_64-unknown-linux-gnu/release:$GST_PLUGIN_PATH"

# Configuration
LISTEN_PORT="${1:-5000}"
STAMPER_TYPE="${2:-fast-robust}"  # Must match publisher!
USE_HW_DECODE="${3:-true}"  # true to use hardware decoding

echo "========================================================"
echo "NVIDIA Hardware H.264 Subscriber"
echo "========================================================"
echo "Listen Port: $LISTEN_PORT"
echo "Stamper Type: $STAMPER_TYPE (must match publisher)"
echo "Hardware Decode: $USE_HW_DECODE"
echo ""
echo "Usage: $0 [port] [stamper-type] [use-hw-decode]"
echo "Example: $0 5000 fast-robust true"
echo "========================================================"

# Check for decoders
HW_DECODER_AVAILABLE=false
if gst-inspect-1.0 nvh264dec &>/dev/null; then
    HW_DECODER_AVAILABLE=true
    HW_DECODER="nvh264dec"
elif gst-inspect-1.0 nvh264sldec &>/dev/null; then
    HW_DECODER_AVAILABLE=true
    HW_DECODER="nvh264sldec"
fi

# Select decoder
if [ "$USE_HW_DECODE" = "true" ] && [ "$HW_DECODER_AVAILABLE" = "true" ]; then
    DECODER=$HW_DECODER
    echo "Decoder: $HW_DECODER (NVIDIA hardware)"
elif gst-inspect-1.0 avdec_h264 &>/dev/null; then
    DECODER="avdec_h264"
    echo "Decoder: avdec_h264 (software)"
elif gst-inspect-1.0 openh264dec &>/dev/null; then
    DECODER="openh264dec"
    echo "Decoder: openh264dec (software)"
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

if [ "$DECODER" = "$HW_DECODER" ] && [ "$HW_DECODER" = "nvh264dec" ]; then
    # Hardware decoder with CUDA download
    gst-launch-1.0 -v \
        udpsrc port=$LISTEN_PORT caps="application/x-rtp,media=video,clock-rate=90000,encoding-name=H264,payload=96" \
        ! rtph264depay \
        ! h264parse \
        ! $DECODER \
        ! cudadownload \
        ! tslatencymeasure stamper-type=$STAMPER_TYPE \
        ! videoconvert \
        ! autovideosink sync=false
else
    # Standard pipeline
    gst-launch-1.0 -v \
        udpsrc port=$LISTEN_PORT caps="application/x-rtp,media=video,clock-rate=90000,encoding-name=H264,payload=96" \
        ! rtph264depay \
        ! h264parse \
        ! $DECODER \
        ! tslatencymeasure stamper-type=$STAMPER_TYPE \
        ! videoconvert \
        ! autovideosink sync=false
fi