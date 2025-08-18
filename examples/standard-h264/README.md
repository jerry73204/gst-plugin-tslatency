# Standard H.264 Example

This example uses standard software-based H.264 encoding and decoding that works on any system with GStreamer plugins installed.

## Requirements
- `x264enc` - H.264 software encoder
- `avdec_h264` or `openh264dec` - H.264 software decoder

Install with:
```bash
sudo apt install gstreamer1.0-plugins-ugly gstreamer1.0-libav
```

## Usage

### Terminal 1: Start Subscriber
```bash
./sub.sh [port] [stamper-type]
# Example:
./sub.sh 5000 fast-robust
```

### Terminal 2: Start Publisher
```bash
./pub.sh [host] [port] [stamper-type]
# Example:
./pub.sh 127.0.0.1 5000 fast-robust
```

## Features
- Software-based encoding/decoding (works everywhere)
- RTP streaming over UDP
- Configurable bitrate and latency settings
- Support for all stamper types (original, optimized, fast-robust)

## Performance
- CPU intensive (uses software encoding)
- Suitable for testing and systems without hardware acceleration
- Typical latency: 50-200ms depending on CPU