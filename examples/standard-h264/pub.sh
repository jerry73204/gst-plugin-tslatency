#!/usr/bin/env bash
set -e

# Standard H.264 Publisher using x264enc (software encoder)
# This works on any system with GStreamer plugins installed

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
STAMPER_TYPE="${3:-optimized}"  # original, optimized, or fast-robust

echo "========================================================"
echo "Standard H.264 Publisher (Software Encoder)"
echo "========================================================"
echo "Destination: $DEST_HOST:$DEST_PORT"
echo "Stamper Type: $STAMPER_TYPE"
echo "Encoder: x264enc (CPU-based)"
echo ""
echo "Usage: $0 [host] [port] [stamper-type]"
echo "Example: $0 192.168.1.100 5000 fast-robust"
echo "========================================================"

# Check for x264enc
if ! gst-inspect-1.0 x264enc &>/dev/null; then
    echo "ERROR: x264enc not found!"
    echo "Install with: sudo apt install gstreamer1.0-plugins-ugly"
    exit 1
fi

echo "Starting H.264 stream..."

gst-launch-1.0 -v \
    videotestsrc pattern=smpte \
    ! 'video/x-raw,width=1920,height=1080,format=I420,framerate=30/1' \
    ! tslatencystamper stamper-type=$STAMPER_TYPE \
    ! x264enc tune=zerolatency bitrate=2000 key-int-max=30 \
    ! rtph264pay config-interval=1 pt=96 \
    ! udpsink host=$DEST_HOST port=$DEST_PORT