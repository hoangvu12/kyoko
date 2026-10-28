# 93 — Images: fetch visible ones first and in parallel like Chrome

**What to build:** Follow-up from 79. The media pipeline fetches at most 6 images at a time and doesn't run them in the order asked (each first checks the disk cache on a background thread), so a picker or a channel full of images fills in slowly: about 60 visible emoji at 120 ms each take ~1.2 s on mockcord's crowded mode. Chrome sends many image requests at once over one HTTP/2 connection to the CDN. Match that: visible images first, in request order, with HTTP/2 multiplexing.

**Blocked by:** none

**Status:** claimed

- [ ] Measure first with mockcord's `--crowded-expressions` and `media_delay` (79's rig): first-rows time for the emoji picker and for a channel with many images
- [ ] Requests to the same CDN host share one HTTP/2 connection with a higher in-flight limit (pick a number that matches what Chrome does for images on one origin; write the source); disk-cache checks don't hold a network slot
- [ ] Visible images are fetched before off-screen ones, in the order the UI asked; images that scrolled away before their fetch started are dropped from the queue
- [ ] TLS seam tests still match Chrome (one connection, image weight 147 and header order unchanged); before/after numbers in the Answer
