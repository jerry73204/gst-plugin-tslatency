#!/usr/bin/env bash
set -e

# Self-contained test using NVIDIA DeepStream SDK
# Tests nvv4l2 encoding and decoding in a single pipeline

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
project_root="$script_dir/../.."

# Build the plugin
cd "$project_root"
cargo cbuild --release

# Set up plugin path
export GST_PLUGIN_PATH="$project_root/target/x86_64-unknown-linux-gnu/release:$GST_PLUGIN_PATH"
export GST_DEBUG=tslatencystamper:4,tslatencymeasure:4

# DeepStream paths
DEEPSTREAM_PATH="/opt/nvidia/deepstream/deepstream"
if [ -d "$DEEPSTREAM_PATH" ]; then
    export LD_LIBRARY_PATH="$DEEPSTREAM_PATH/lib/:$LD_LIBRARY_PATH"
    export GST_PLUGIN_PATH="$DEEPSTREAM_PATH/lib/gst-plugins/:$GST_PLUGIN_PATH"
fi

# Configuration
STAMPER_TYPE="${1:-fast-robust}"
DURATION="${2:-100}"  # Number of frames
CODEC="${3:-h265}"  # h264 or h265

echo "========================================================"
echo "NVIDIA DeepStream Self-Test (Single Pipeline)"
echo "========================================================"
echo "Stamper Type: $STAMPER_TYPE"
echo "Duration: $DURATION frames"
echo "Codec: $CODEC"
echo ""
echo "This tests nvv4l2enc → nvv4l2dec latency in one pipeline"
echo "Usage: $0 [stamper-type] [num-frames] [h264|h265]"
echo "========================================================"

# Check for DeepStream elements
if ! gst-inspect-1.0 nvvideoconvert &>/dev/null; then
    echo "WARNING: DeepStream elements not found!"
    echo "Falling back to CUDA/NVENC test..."
    exec "$script_dir/../cuda-nvenc/self.sh" "$@"
fi

echo "DeepStream elements found!"

# Select encoder/decoder based on codec
if [ "$CODEC" = "h264" ]; then
    ENCODER="nvv4l2h264enc bitrate=4000000 preset-id=1 insert-sps-pps=1 iframeinterval=30"
    PARSER="h264parse"
    echo "Using nvv4l2 H.264 codecs"
else
    ENCODER="nvv4l2h265enc bitrate=4000000 preset-id=1 iframeinterval=30"
    PARSER="h265parse"
    echo "Using nvv4l2 H.265 codecs"
fi

# nvv4l2decoder is universal for both H.264 and H.265
DECODER="nvv4l2decoder"

echo "========================================================"
echo "Running test..."
echo ""

# Run self-contained pipeline with NVMM memory
gst-launch-1.0 -v \
    videotestsrc pattern=smpte num-buffers=$DURATION \
    ! 'video/x-raw,width=1920,height=1080,format=I420,framerate=30/1' \
    ! tslatencystamper stamper-type=$STAMPER_TYPE \
    ! nvvideoconvert \
    ! 'video/x-raw(memory:NVMM),format=NV12' \
    ! $ENCODER \
    ! $PARSER \
    ! $DECODER \
    ! nvvideoconvert \
    ! 'video/x-raw,format=I420' \
    ! tslatencymeasure stamper-type=$STAMPER_TYPE \
    ! videoconvert \
    ! autovideosink sync=false

echo ""
echo "Test completed!"