# DeepStream Example

This example uses NVIDIA DeepStream SDK elements for hardware-accelerated video processing.

## Requirements
- NVIDIA GPU with Video Codec SDK support
- NVIDIA DeepStream SDK 6.0+ installed (tested with 7.1)
- NVIDIA drivers 470+

## Installing DeepStream

1. **Download DeepStream SDK**
   - Visit: https://developer.nvidia.com/deepstream-sdk
   - Download the .deb package for Ubuntu 22.04
   - Requires free NVIDIA Developer account

2. **Install DeepStream**
   ```bash
   sudo apt-get update
   sudo apt-get install ./deepstream-6.4_6.4.0-1_amd64.deb
   ```

3. **Set Environment Variables**
   Add to `~/.bashrc`:
   ```bash
   export LD_LIBRARY_PATH=/opt/nvidia/deepstream/deepstream/lib/:$LD_LIBRARY_PATH
   export GST_PLUGIN_PATH=/opt/nvidia/deepstream/deepstream/lib/gst-plugins/:$GST_PLUGIN_PATH
   ```

4. **Verify Installation**
   ```bash
   gst-inspect-1.0 nvvideoconvert
   gst-inspect-1.0 nvv4l2h264enc
   gst-inspect-1.0 nvv4l2decoder
   ```

## DeepStream Elements
- `nvvideoconvert` - Hardware-accelerated color conversion and scaling
- `nvv4l2h264enc` - V4L2-based H.264 encoder (uses NVENC)
- `nvv4l2h265enc` - V4L2-based H.265 encoder (uses NVENC)
- `nvv4l2decoder` - V4L2-based decoder (uses NVDEC)
- NVMM memory - NVIDIA Multimedia Memory for zero-copy operations

### Encoder Presets (preset-id)
- **1**: P1 - Highest performance, lowest quality
- **2**: P2 - Very high performance
- **3**: P3 - High performance
- **4**: P4 - Balanced (default)
- **5**: P5 - Better quality
- **6**: P6 - High quality
- **7**: P7 - Highest quality, lowest performance

## Usage

### Terminal 1: Start Subscriber
```bash
./sub.sh [port] [codec] [stamper-type]
# Example for H.265:
./sub.sh 5000 h265 fast-robust

# Example for H.264:
./sub.sh 5000 h264 optimized
```

### Terminal 2: Start Publisher
```bash
./pub.sh [host] [port] [codec] [stamper-type]
# Example for H.265:
./pub.sh 127.0.0.1 5000 h265 fast-robust

# Example for H.264:
./pub.sh 127.0.0.1 5000 h264 optimized
```

## Features
- **NVMM Memory**: Zero-copy path through NVIDIA Multimedia Memory
- **V4L2 Interface**: Standardized Linux video interface
- **Multiple Codecs**: Support for both H.264 and H.265
- **Automatic Fallback**: Falls back to cuda-nvenc if DeepStream not available

## Performance
- **Encoding**: Hardware-accelerated with minimal CPU usage
- **Memory**: Zero-copy through NVMM reduces memory bandwidth
- **Latency**: Ultra-low latency (5-20ms typical)
- **Throughput**: Can handle multiple 4K streams simultaneously

## Troubleshooting

### Elements Not Found
If DeepStream elements are not found after installation:
```bash
# Clear GStreamer cache
rm -rf ~/.cache/gstreamer-1.0/

# Rebuild registry
gst-inspect-1.0 --gst-disable-segtrap

# Check DeepStream installation
ls /opt/nvidia/deepstream/deepstream/lib/gst-plugins/
```

### Fallback Behavior
The scripts automatically fall back to cuda-nvenc examples if DeepStream is not available, ensuring the examples work even without DeepStream installed.