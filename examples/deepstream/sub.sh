#!/usr/bin/env bash
set -e

# NVIDIA DeepStream H.264/H.265 Subscriber
# Requires NVIDIA DeepStream SDK installed

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
project_root="$script_dir/../.."

# Build the plugin
cd "$project_root"
cargo cbuild --release

# Set up plugin path
export GST_PLUGIN_PATH="$project_root/target/x86_64-unknown-linux-gnu/release:$GST_PLUGIN_PATH"

# DeepStream paths (adjust if installed elsewhere)
DEEPSTREAM_PATH="/opt/nvidia/deepstream/deepstream"
if [ -d "$DEEPSTREAM_PATH" ]; then
    export LD_LIBRARY_PATH="$DEEPSTREAM_PATH/lib/:$LD_LIBRARY_PATH"
    export GST_PLUGIN_PATH="$DEEPSTREAM_PATH/lib/gst-plugins/:$GST_PLUGIN_PATH"
fi

# Configuration
LISTEN_PORT="${1:-5000}"
CODEC="${2:-h265}"  # h264 or h265
STAMPER_TYPE="${3:-fast-robust}"  # Must match publisher!

echo "========================================================"
echo "NVIDIA DeepStream Subscriber"
echo "========================================================"
echo "Listen Port: $LISTEN_PORT"
echo "Codec: $CODEC"
echo "Stamper Type: $STAMPER_TYPE (must match publisher)"
echo ""
echo "Usage: $0 [port] [h264|h265] [stamper-type]"
echo "Example: $0 5000 h265 fast-robust"
echo "========================================================"

# Check for DeepStream elements
if ! gst-inspect-1.0 nvvideoconvert &>/dev/null; then
    echo "WARNING: DeepStream elements not found!"
    echo "nvvideoconvert is not available."
    echo ""
    echo "To install DeepStream:"
    echo "1. Download DeepStream SDK from NVIDIA Developer site"
    echo "2. Install the .deb package"
    echo "3. Set environment variables (add to ~/.bashrc):"
    echo "   export LD_LIBRARY_PATH=/opt/nvidia/deepstream/deepstream/lib/:\$LD_LIBRARY_PATH"
    echo "   export GST_PLUGIN_PATH=/opt/nvidia/deepstream/deepstream/lib/gst-plugins/:\$GST_PLUGIN_PATH"
    echo ""
    echo "Falling back to non-DeepStream pipeline..."
    echo ""
    
    # Fallback to cuda/nvenc pipeline
    exec "$script_dir/../cuda-nvenc/sub.sh" "$@"
fi

echo "DeepStream elements found!"
echo ""

# Enable latency measurement debug
export GST_DEBUG=tslatencymeasure:4

# Select decoder based on codec
if [ "$CODEC" = "h264" ]; then
    CAPS="application/x-rtp,media=video,clock-rate=90000,encoding-name=H264,payload=96"
    DEPAYLOADER="rtph264depay"
    PARSER="h264parse"
    DECODER="nvv4l2decoder"
else
    CAPS="application/x-rtp,media=video,clock-rate=90000,encoding-name=H265,payload=96"
    DEPAYLOADER="rtph265depay"
    PARSER="h265parse"
    DECODER="nvv4l2decoder"
fi

echo "Waiting for DeepStream $CODEC stream on port $LISTEN_PORT..."
echo ""

gst-launch-1.0 -v \
    udpsrc port=$LISTEN_PORT caps="$CAPS" \
    ! $DEPAYLOADER \
    ! $PARSER \
    ! $DECODER \
    ! nvvideoconvert \
    ! 'video/x-raw,format=I420' \
    ! tslatencymeasure stamper-type=$STAMPER_TYPE \
    ! videoconvert \
    ! autovideosink sync=false