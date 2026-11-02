# Coconut thumbnails

`coconut-thumbnails` generates bounded PNG previews for images (PNG, JPEG,
WebP, GIF, BMP, ICO, TIFF and SVG), videos (MP4, MKV, WebM, MOV and AVI),
embedded audio artwork (MP3, M4A, FLAC, OGG, Opus, WMA, WAV and AAC), and
embedded PE icon resources (`.exe`). `Thumbnailer::get(path, side)` returns
PNG bytes or `None` when the format has no preview. Embedded audio metadata
is read natively with Symphonia; `ffmpeg` is an optional fallback for artwork
Symphonia cannot extract. Video frames use `ffmpeg` first and then
`gst-launch-1.0` (GStreamer) when FFmpeg is unavailable or cannot decode the
file. The GStreamer backend uses the locally installed demuxer/decoder and
PNG plugins, so supported codecs depend on the system. Both backends have
bounded output and a five-second process timeout; GStreamer receives only
`file://` URIs. If neither backend can decode the video, or an audio file
has no embedded cover, consumers use their ordinary MIME icon. Image and PE
decoding are in-process. Consumers should call `get` on a worker.

The freedesktop.org thumbnail cache lives under `$XDG_CACHE_HOME/thumbnails`
and uses the MD5 of a file URI as the PNG filename. Coconut instead keeps a
private cache under `$XDG_CACHE_HOME/coconut/thumbnails` so its contents do
not expose photos or file paths. Filenames are HMAC-SHA256 of the canonical
absolute path, size, modification timestamp and requested preview size. PNG
bytes are encrypted with AES-256-GCM and a random nonce. The random 256-bit
key is stored separately at `$XDG_STATE_HOME/coconut/thumbnails.key`; new
directories and files use mode 0700 and 0600 respectively. Missing or
tampered entries regenerate from the source. No metadata is written to
the original directory. If a generated preview cannot be saved to the cache,
it is still returned to the caller and the cache error is logged.

The cache retains up to 256 MiB of encrypted entries and 8,192 entries. Entries
unused for 30 days expire. Opening the cache and subsequent writes check for
hourly maintenance; hitting a preview refreshes its recency at most once an
hour. When a quota is reached, the oldest entries are removed in batches to
leave room for new previews. Evicted previews regenerate from the original file.

A mode-0600 `thumbnails.lock` file beside the cache serializes maintenance and
writes across applications. Its small usage record avoids directory scans on
ordinary writes; directory changes or interrupted accounting trigger a fresh
scan. Cleanup failure is logged and does not discard a generated preview.
Cleanup's candidate storage is bounded by the entry quota even when opening
an oversized cache.

This is not interoperable with other desktops' PNG thumbnail caches. The key
is accessible to processes running as the same user, so this protects an
isolated copy of the cache, not against an attacker who can also read the
user's state directory or process memory. Deleting the key invalidates all
existing cached previews.
