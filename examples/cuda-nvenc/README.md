# CUDA/NVENC Example

This example uses NVIDIA hardware acceleration for H.264 encoding and decoding using NVENC/NVDEC through the nvcodec GStreamer plugin.

## Requirements
- NVIDIA GPU with NVENC support (GTX 600 series or newer)
- NVIDIA drivers (version 470+)
- GStreamer nvcodec plugin

Check available elements:
```bash
gst-inspect-1.0 | grep nvcodec
```

## Available Elements
- `nvh264enc` - NVIDIA hardware H.264 encoder (NVENC)
- `nvh265enc` - NVIDIA hardware H.265 encoder (NVENC)
- `nvh264dec` - NVIDIA hardware H.264 decoder (NVDEC)
- `cudaupload` - Upload frames to GPU memory
- `cudadownload` - Download frames from GPU memory
- `cudaconvert` - GPU-accelerated color conversion

## Usage

### Terminal 1: Start Subscriber
```bash
./sub.sh [port] [stamper-type] [use-hw-decode]
# Example with hardware decoding:
./sub.sh 5000 fast-robust true

# Example with software decoding:
./sub.sh 5000 fast-robust false
```

### Terminal 2: Start Publisher
```bash
./pub.sh [host] [port] [stamper-type] [use-cuda-memory]
# Example with CUDA memory (best performance):
./pub.sh 127.0.0.1 5000 fast-robust true

# Example without CUDA memory:
./pub.sh 127.0.0.1 5000 fast-robust false
```

## Performance Notes
- **CUDA Memory Path**: Keeps video data in GPU memory for maximum performance
- **Hardware encoding**: ~5-10x faster than software with minimal CPU usage
- **Low latency preset**: Optimized for real-time streaming
- **Typical latency**: 10-50ms with hardware acceleration

## Troubleshooting
If NVIDIA elements are not found:
1. Check NVIDIA driver: `nvidia-smi`
2. Verify CUDA installation: `nvcc --version`
3. Rebuild GStreamer bad plugins with nvcodec support
4. Check GPU capabilities: `nvidia-smi -q | grep Video`