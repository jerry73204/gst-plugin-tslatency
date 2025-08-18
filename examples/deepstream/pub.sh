#!/usr/bin/env bash
set -e

# NVIDIA DeepStream H.264/H.265 Publisher
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
DEST_HOST="${1:-127.0.0.1}"
DEST_PORT="${2:-5000}"
CODEC="${3:-h265}"  # h264 or h265
STAMPER_TYPE="${4:-fast-robust}"

echo "========================================================"
echo "NVIDIA DeepStream Publisher"
echo "========================================================"
echo "Destination: $DEST_HOST:$DEST_PORT"
echo "Codec: $CODEC"
echo "Stamper Type: $STAMPER_TYPE"
echo ""
echo "Usage: $0 [host] [port] [h264|h265] [stamper-type]"
echo "Example: $0 192.168.1.100 5000 h265 fast-robust"
echo "========================================================"

# Check for DeepStream elements
if ! gst-inspect-1.0 nvvideoconvert &>/dev/null; then
    echo "WARNING: DeepStream elements not found!"
    echo "nvvideoconvert is not available."
    echo ""
    echo "To install DeepStream:"
    echo "1. Download DeepStream SDK from NVIDIA Developer site"
    echo "2. Install the .deb package"
    echo "3. Set environment variables:"
    echo "   export LD_LIBRARY_PATH=/opt/nvidia/deepstream/deepstream/lib/:\$LD_LIBRARY_PATH"
    echo "   export GST_PLUGIN_PATH=/opt/nvidia/deepstream/deepstream/lib/gst-plugins/:\$GST_PLUGIN_PATH"
    echo ""
    echo "Falling back to non-DeepStream pipeline..."
    echo ""
    
    # Fallback to cuda/nvenc pipeline
    exec "$script_dir/../cuda-nvenc/pub.sh" "$@"
fi

echo "DeepStream elements found!"
echo "Available DeepStream elements:"
gst-inspect-1.0 2>/dev/null | grep -E "(nvvideoconvert|nvv4l2)" | head -10
echo ""

# Select encoder based on codec
if [ "$CODEC" = "h264" ]; then
    ENCODER="nvv4l2h264enc bitrate=4000000 preset-id=1 insert-sps-pps=1 iframeinterval=30"
    PAYLOADER="rtph264pay config-interval=1 pt=96"
    CAPS="application/x-rtp,media=video,clock-rate=90000,encoding-name=H264,payload=96"
else
    ENCODER="nvv4l2h265enc bitrate=4000000 preset-id=1 iframeinterval=30"
    PAYLOADER="rtph265pay config-interval=1 pt=96"
    CAPS="application/x-rtp,media=video,clock-rate=90000,encoding-name=H265,payload=96"
fi

echo "Starting DeepStream $CODEC stream..."

gst-launch-1.0 -v \
    videotestsrc pattern=smpte \
    ! 'video/x-raw,width=1920,height=1080,format=I420,framerate=30/1' \
    ! tslatencystamper stamper-type=$STAMPER_TYPE \
    ! nvvideoconvert \
    ! 'video/x-raw(memory:NVMM),format=NV12' \
    ! $ENCODER \
    ! $PAYLOADER \
    ! udpsink host=$DEST_HOST port=$DEST_PORT