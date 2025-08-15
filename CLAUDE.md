# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a GStreamer plugin written in Rust that measures pipeline latency by adding and reading binary timestamps on video frames. The plugin consists of two main elements:
- **tslatencystamper**: Stamps binary timestamps on video frames
- **tslatencymeasure**: Reads the timestamps and calculates latency

## Build Commands

```bash
# Build the plugin (standard build)
cargo build --release

# Build the C-compatible plugin (required for GStreamer integration)
cargo cbuild --release

# Check code for errors without building
cargo check

# Run linter
cargo clippy

# Format code
cargo fmt
```

## Testing and Running

The project includes example scripts in the `scripts/` directory:

```bash
# Run sender/receiver test (modify IP in pub.sh first)
./scripts/pub.sh  # Sender side
./scripts/sub.sh  # Receiver side

# Run self-contained test (encoder->decoder in single pipeline)
./scripts/self.sh

# NVIDIA-accelerated versions (if using DeepStream)
./scripts/nv_pub.sh
./scripts/nv_sub.sh
```

Before running scripts, ensure GST_PLUGIN_PATH includes the built plugin:
```bash
export GST_PLUGIN_PATH="$PWD/target/x86_64-unknown-linux-gnu/release:$GST_PLUGIN_PATH"
```

## Architecture

The plugin follows GStreamer's plugin architecture with Rust bindings:

- **src/lib.rs**: Plugin registration and initialization
- **src/tslatencystamper/**: Stamps timestamps on frames
  - Encodes current time as binary pattern in configurable region (default 64x64 at 0,0)
  - Supports various RGB and YUV video formats
- **src/tslatencymeasure/**: Reads timestamps and measures latency
  - Decodes binary pattern from frames
  - Calculates latency between stamp time and current time
  - Configurable tolerance for pattern recognition

Both elements extend GStreamer's VideoFilter base class and operate as in-place transforms on video frames. They share similar property interfaces for configuring the timestamp region (x, y, width, height).

## Key Dependencies

- GStreamer 1.0 and its video subsystem
- Rust GStreamer bindings (gstreamer, gstreamer-video, gstreamer-base)
- cargo-c for building C-compatible shared libraries