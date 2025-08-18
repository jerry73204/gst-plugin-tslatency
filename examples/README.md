# Timestamp Latency Examples

This directory contains example publisher/subscriber pairs demonstrating different encoding technologies with the timestamp latency plugin.

## Directory Structure

```
examples/
├── standard-h264/    # Software-based H.264 encoding
│   ├── pub.sh       # Publisher using x264enc
│   ├── sub.sh       # Subscriber using avdec_h264
│   └── README.md    
│
├── cuda-nvenc/      # NVIDIA hardware acceleration (nvcodec)
│   ├── pub.sh       # Publisher using nvh264enc
│   ├── sub.sh       # Subscriber using nvh264dec
│   └── README.md    
│
└── deepstream/      # NVIDIA DeepStream SDK
    ├── pub.sh       # Publisher using nvv4l2h264enc/h265enc
    ├── sub.sh       # Subscriber using nvv4l2decoder
    └── README.md    
```

## Quick Start

Each example follows the same pattern:

1. **Start the subscriber** (receiver) first:
   ```bash
   cd examples/[example-dir]
   ./sub.sh
   ```

2. **Start the publisher** (sender) in another terminal:
   ```bash
   cd examples/[example-dir]
   ./pub.sh
   ```

## Comparison

| Example | Requirements | CPU Usage | GPU Usage | Latency | Quality |
|---------|-------------|-----------|-----------|---------|---------|
| **standard-h264** | GStreamer plugins | High | None | 50-200ms | Good |
| **cuda-nvenc** | NVIDIA GPU + drivers | Low | Medium | 10-50ms | Excellent |
| **deepstream** | DeepStream SDK | Very Low | High | 5-20ms | Excellent |

## Choosing an Example

### Use `standard-h264` when:
- Testing on any system without special hardware
- GPU is not available
- Maximum compatibility is needed
- CPU resources are available

### Use `cuda-nvenc` when:
- NVIDIA GPU is available
- Low latency is important
- CPU resources should be preserved
- You don't have DeepStream SDK

### Use `deepstream` when:
- Building production NVIDIA-accelerated pipelines
- Ultra-low latency is critical
- Processing multiple streams
- Full DeepStream SDK features are needed

## Common Parameters

All examples support these parameters:

- **Stamper Types**: `original`, `optimized`, `fast-robust`
  - `original`: Simple, no error correction
  - `optimized`: CRC validation, good for moderate compression
  - `fast-robust`: BCH error correction, best for heavy compression

- **Ports**: Default is 5000, configurable
- **Hosts**: Default is 127.0.0.1 (localhost)

## Testing Latency

The subscriber scripts enable debug output to show measured latencies:
```
Delay XXX usecs
```

This shows the time difference between when the frame was stamped (publisher) and when it was read (subscriber).

## Troubleshooting

### No Video Output
- Check firewall settings for UDP port (default 5000)
- Ensure publisher and subscriber use the same stamper-type
- Verify network connectivity between hosts

### High Latency
- Check network bandwidth and congestion
- Try hardware acceleration (cuda-nvenc or deepstream)
- Reduce video resolution or framerate
- Adjust encoder presets for lower latency

### Missing Elements
- standard-h264: `sudo apt install gstreamer1.0-plugins-ugly gstreamer1.0-libav`
- cuda-nvenc: Requires NVIDIA drivers and gstreamer1.0-plugins-bad
- deepstream: Requires DeepStream SDK installation

## Advanced Usage

### Custom Pipeline Testing
You can modify the scripts to test different:
- Resolutions: Change width/height in caps
- Framerates: Adjust framerate in caps
- Bitrates: Modify encoder bitrate parameters
- Patterns: Change videotestsrc pattern
- Network protocols: Replace UDP with TCP, RTSP, etc.