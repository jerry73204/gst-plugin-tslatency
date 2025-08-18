#!/usr/bin/env bash
set -e

# NVIDIA Hardware-Accelerated H.264 Publisher using NVENC
# Requires NVIDIA GPU with NVENC support

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
project_root="$script_dir/../.."

# Build the plugin
cd "$project_root"
cargo cbuild --release

# Set up plugin path
export GST_PLUGIN_PATH="$project_root/target/x86_64-unknown-linux-gnu/release:$GST_PLUGIN_PATH"

# Configuration
DEST_HOST="${1:-127.0.0.1}"
DEST_PORT="${2:-5000}"
STAMPER_TYPE="${3:-fast-robust}"  # original, optimized, or fast-robust
USE_CUDA_MEMORY="${4:-false}"  # true to use CUDA memory path

echo "========================================================"
echo "NVIDIA Hardware H.264 Publisher (NVENC)"
echo "========================================================"
echo "Destination: $DEST_HOST:$DEST_PORT"
echo "Stamper Type: $STAMPER_TYPE"
echo "Use CUDA Memory: $USE_CUDA_MEMORY"
echo ""
echo "Usage: $0 [host] [port] [stamper-type] [use-cuda-memory]"
echo "Example: $0 192.168.1.100 5000 fast-robust true"
echo "========================================================"

# Check for NVIDIA elements
if ! gst-inspect-1.0 nvh264enc &>/dev/null; then
    echo "ERROR: nvh264enc not found!"
    echo "NVIDIA hardware encoding is not available."
    echo "Make sure you have:"
    echo "  - NVIDIA GPU with NVENC support"
    echo "  - NVIDIA drivers installed"
    echo "  - gstreamer1.0-plugins-bad with nvcodec support"
    exit 1
fi

echo "Available NVIDIA elements:"
gst-inspect-1.0 2>/dev/null | grep "nvcodec:" | head -10
echo ""

# Test if CUDA memory works
CUDA_WORKS=false
if [ "$USE_CUDA_MEMORY" = "true" ]; then
    if gst-inspect-1.0 cudaupload &>/dev/null && \
       GST_DEBUG=0 gst-launch-1.0 videotestsrc num-buffers=1 ! cudaupload ! cudaconvert ! cudadownload ! fakesink 2>/dev/null; then
        CUDA_WORKS=true
    else
        echo "WARNING: CUDA memory operations not working, using standard pipeline"
        echo "Error: CUDA architecture mismatch or missing CUDA runtime"
        USE_CUDA_MEMORY=false
    fi
fi

if [ "$USE_CUDA_MEMORY" = "true" ] && [ "$CUDA_WORKS" = "true" ]; then
    echo "Using CUDA memory pipeline (maximum performance)..."
    
    gst-launch-1.0 -v \
        videotestsrc pattern=smpte \
        ! 'video/x-raw,width=1920,height=1080,format=I420,framerate=30/1' \
        ! tslatencystamper stamper-type=$STAMPER_TYPE \
        ! cudaupload \
        ! cudaconvert \
        ! 'video/x-raw(memory:CUDAMemory),format=NV12' \
        ! nvh264enc preset=low-latency-hq bitrate=4000 gop-size=30 \
        ! h264parse \
        ! rtph264pay config-interval=1 pt=96 \
        ! udpsink host=$DEST_HOST port=$DEST_PORT
else
    echo "Using standard memory pipeline (still hardware-accelerated)..."
    
    gst-launch-1.0 -v \
        videotestsrc pattern=smpte \
        ! 'video/x-raw,width=1920,height=1080,format=I420,framerate=30/1' \
        ! tslatencystamper stamper-type=$STAMPER_TYPE \
        ! videoconvert \
        ! nvh264enc preset=low-latency-hq bitrate=4000 gop-size=30 \
        ! h264parse \
        ! rtph264pay config-interval=1 pt=96 \
        ! udpsink host=$DEST_HOST port=$DEST_PORT
fi