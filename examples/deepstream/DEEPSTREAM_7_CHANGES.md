# DeepStream 7.x Property Changes

## nvv4l2h264enc / nvv4l2h265enc

### Changed Properties
| Old (DS 6.x) | New (DS 7.x) | Description |
|--------------|--------------|-------------|
| `preset-level` | `preset-id` | Performance/quality preset (1-7) |

### Common Properties (DS 7.x)
```bash
# Encoding properties
bitrate=4000000         # Target bitrate in bits/sec
preset-id=1             # 1=fastest, 7=best quality
iframeinterval=30       # I-frame interval
idrinterval=30          # IDR frame interval
control-rate=1          # 0=constant QP, 1=CBR, 2=VBR

# H.264 specific
insert-sps-pps=1        # Insert SPS/PPS with IDR frames

# Performance
gpu-id=0                # GPU device ID

# Quality control
aq=0                    # Adaptive quantization (0=auto, 1-15)
cq=0                    # Constant quality for VBR (0=auto, 1-51)
```

### Example Pipelines

#### H.264 Low Latency
```bash
nvv4l2h264enc \
  bitrate=4000000 \
  preset-id=1 \
  insert-sps-pps=1 \
  iframeinterval=30
```

#### H.265 High Quality
```bash
nvv4l2h265enc \
  bitrate=8000000 \
  preset-id=5 \
  iframeinterval=60 \
  control-rate=2
```

## nvvideoconvert

No significant changes between DS 6.x and 7.x.

### Common Properties
```bash
gpu-id=0                # GPU device ID
nvbuf-memory-type=0     # Memory type (0=default, 1=CUDA, 2=NVMM)
```

## Checking Properties

To see all available properties for any element:
```bash
gst-inspect-1.0 nvv4l2h264enc
gst-inspect-1.0 nvv4l2h265enc
gst-inspect-1.0 nvvideoconvert
```

## Compatibility Note

Scripts using `preset-level` will fail with DeepStream 7.x. Always use `preset-id` for DS 7.x compatibility.