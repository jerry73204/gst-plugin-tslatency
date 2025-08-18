#!/usr/bin/env bash
set -e

# Self-contained test using NVIDIA hardware acceleration
# Tests NVENC encoding and NVDEC decoding in a single pipeline

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
project_root="$script_dir/../.."

# Build the plugin
cd "$project_root"
cargo cbuild --release

# Set up plugin path
export GST_PLUGIN_PATH="$project_root/target/x86_64-unknown-linux-gnu/release:$GST_PLUGIN_PATH"
export GST_DEBUG=tslatencystamper:4,tslatencymeasure:4

# Configuration
STAMPER_TYPE="${1:-fast-robust}"
DURATION="${2:-100}"  # Number of frames
CODEC="${3:-h264}"  # h264 or h265

echo "========================================================"
echo "NVIDIA Hardware Self-Test (Single Pipeline)"
echo "========================================================"
echo "Stamper Type: $STAMPER_TYPE"
echo "Duration: $DURATION frames"
echo "Codec: $CODEC"
echo ""
echo "This tests NVENC → NVDEC latency in one pipeline"
echo "Usage: $0 [stamper-type] [num-frames] [h264|h265]"
echo "========================================================"

# Check for NVIDIA elements
if ! gst-inspect-1.0 nvh264enc &>/dev/null; then
    echo "ERROR: NVIDIA hardware encoding not available!"
    echo "Falling back to software test..."
    exec "$script_dir/../standard-h264/self.sh" "$@"
fi

# Select encoder/decoder based on codec
if [ "$CODEC" = "h265" ]; then
    if ! gst-inspect-1.0 nvh265enc &>/dev/null || ! gst-inspect-1.0 nvh265dec &>/dev/null; then
        echo "H.265 hardware codec not available, using H.264"
        CODEC="h264"
    fi
fi

if [ "$CODEC" = "h265" ]; then
    ENCODER="nvh265enc preset=low-latency-hq bitrate=4000 gop-size=30"
    DECODER="nvh265dec"
    PARSER="h265parse"
    echo "Using H.265 hardware codecs"
else
    ENCODER="nvh264enc preset=low-latency-hq bitrate=4000 gop-size=30"
    DECODER="nvh264dec"
    PARSER="h264parse"
    echo "Using H.264 hardware codecs"
fi

echo "========================================================"
echo "Running test..."
echo ""

# Check if CUDA memory operations are available
if gst-inspect-1.0 cudaupload &>/dev/null && \
   GST_DEBUG=0 gst-launch-1.0 videotestsrc num-buffers=1 ! cudaupload ! cudaconvert ! cudadownload ! fakesink 2>/dev/null; then
    echo "Using CUDA memory pipeline..."
    # Run with CUDA memory
    gst-launch-1.0 -v \
        videotestsrc pattern=smpte num-buffers=$DURATION \
        ! 'video/x-raw,width=1920,height=1080,format=I420,framerate=30/1' \
        ! tslatencystamper stamper-type=$STAMPER_TYPE \
        ! cudaupload \
        ! cudaconvert \
        ! 'video/x-raw(memory:CUDAMemory),format=NV12' \
        ! $ENCODER \
        ! $PARSER \
        ! $DECODER \
        ! cudadownload \
        ! tslatencymeasure stamper-type=$STAMPER_TYPE \
        ! videoconvert \
        ! autovideosink sync=false
else
    echo "CUDA memory operations not available, using direct pipeline..."
    # Run without CUDA memory operations
    gst-launch-1.0 -v \
        videotestsrc pattern=smpte num-buffers=$DURATION \
        ! 'video/x-raw,width=1920,height=1080,format=I420,framerate=30/1' \
        ! tslatencystamper stamper-type=$STAMPER_TYPE \
        ! videoconvert \
        ! $ENCODER \
        ! $PARSER \
        ! $DECODER \
        ! tslatencymeasure stamper-type=$STAMPER_TYPE \
        ! videoconvert \
        ! autovideosink sync=false
fi

echo ""
echo "Test completed!"