# gst-plugin-tslatency

Measure pipeline latency by binary timestamps on video frames.

## Installation

Install `cargo-c` if you haven't done it yet.


```sh
cargo install cargo-c
```

Clone this repository first and go into the directory. Build the
GStreamer plugin.


```sh
cargo cbuild
```


## Demo

Modift the video receiver address.

```sh
vim scripts/pub.sh
# Find "udpsink host=127.0.0.1 port=5000" and change the address.
```

On the sender side, run the publication script.

```sh
./scripts/pub.sh
```

On the receiver side, run the video retrieval script.

```sh
./scripts/sub.sh
```

If the scripts work without errors, the receiver side will prompts a
window, showing the video with binary timestamps on the top left
corner.
