# 93 — Images: fetch visible ones first and in parallel like Chrome

**What to build:** Follow-up from 79. The media pipeline fetches at most 6 images at a time and doesn't run them in the order asked (each first checks the disk cache on a background thread), so a picker or a channel full of images fills in slowly: about 60 visible emoji at 120 ms each take ~1.2 s on mockcord's crowded mode. Chrome sends many image requests at once over one HTTP/2 connection to the CDN. Match that: visible images first, in request order, with HTTP/2 multiplexing.

**Blocked by:** none

**Status:** resolved

- [x] Measure first with mockcord's `--crowded-expressions` and `media_delay` (79's rig): first-rows time for the emoji picker and for a channel with many images
- [x] Requests to the same CDN host share one HTTP/2 connection with a higher in-flight limit (pick a number that matches what Chrome does for images on one origin; write the source); disk-cache checks don't hold a network slot
- [x] Visible images are fetched before off-screen ones, in the order the UI asked; images that scrolled away before their fetch started are dropped from the queue
- [x] TLS seam tests still match Chrome (one connection, image weight 147 and header order unchanged); before/after numbers in the Answer

## Answer

Merged to main (worker `bf851156`, lane C; "Fetch images per host in Chrome's order and limits" and "Time a channel of image grids on mockcord"; full verify re-run by the orchestrator on `e73374f`: green apart from `composer::a_server_gate_points_to_the_official_client_without_a_confirm` timing out once under load (bridge harness wait), which passed 5 of 5 alone; `tls-capture --compare chrome-156` 0 and 0; rebased onto 94 (UI only) without a rerun). Docs: `docs/media.md`, `docs/fingerprint.md`, `docs/testing.md`.

- Before -> after (mockcord, 120 ms per image, three runs): emoji picker crowded first rows 1.31-1.61 s -> 0.31-0.33 s (69 fetches); sticker picker 0.32-0.52 s -> 0.17-0.18 s; a channel with four 10-image grids, on-screen images shown 0.77-0.90 s -> 0.33-0.39 s (new `timeline::fetch_tests`).
- Root cause: one 6-slot counter for all fetches, ignoring host and protocol; slots went to whichever load finished its disk check first; nothing ranked on-screen images or dropped scrolled-away ones; wreq opens a TLS connection per concurrent request to a cold host.
- New `crates/media/src/queue.rs`: per-host queue after the disk cache; while a host's protocol is unknown only one request runs (so a burst opens one connection), then up to 100 on HTTP/2 or 6 on HTTP/1.1. Order: user actions (audio, Download, direct loads), then on-screen images, then the rest in request order. An image reported visible that leaves the screen before its fetch starts is dropped (`Status::Dropped`) and requeued when asked or shown again. `MediaFetcherBuilder::concurrency` became `per_host(http1, http2)`; `MediaFetcher::waiting()`, `protocol(url)`.
- Sources (Chromium main `be3c4d2`, read 2026-10-28): `resource_scheduler.cc` `ShouldStartRequest` doesn't delay requests to servers with request priorities (HTTP/2), `kMaxNumDelayableRequestsPerHostPerClient = 6` otherwise; `spdy_session.h` `kInitialMaxConcurrentStreams = 100`; `spdy_session.cc` caps the server value at 256.
- Mock: mockcord serves media over h2c (`MediaFetcherBuilder::loopback_http2`, axum `http2` feature); `MediaRequest::http2` recorded; own-HTTP/1.1 tests use `only_loopback`.
- Tests: `media_queue::a_burst_of_images_to_one_host_shares_one_http2_connection` (1 TLS connection for 10, weight 147 on each; 10 with the gate off), `media_queue::queued_images_start_visible_first_in_request_order_and_drop_when_they_scroll_away`.
- Deviation: the first request to a host goes alone until its response headers arrive (Chrome only waits for the connection), one extra round trip per host per 90 s idle.
- Known gap now more visible: Chrome chains an overlapping request onto the previous open stream; wreq can't, so kyoko's overlapping image requests all depend on stream 0 at weight 147 (`docs/fingerprint.md`).
