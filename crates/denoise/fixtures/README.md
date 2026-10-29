# Test fixtures

`speech.wav` is test data for `kyoko-denoise`'s `testing` feature (the noise suppression bench and session tests). It is not shipped in the application.

- Source: the LibriVox recording of Abraham Lincoln's *The Gettysburg Address*, read by John Greenman (2006), `gettysburg_address.mp3` from <https://archive.org/details/gettysburg_johng_librivox> (SHA-1 `786074b2ef907d9b11e9fa5962b5e8a3c7dcce79`, as archive.org lists it; SHA-256 `66ffab650fff988fc9ce55457f6112f828815cf37d263dab49eb10ae5a193346`).
- Licence: public domain. LibriVox releases its recordings into the public domain, and the archive.org item carries the Creative Commons public domain dedication (<http://creativecommons.org/licenses/publicdomain/>). The text itself is an 1863 speech in the public domain.
- Excerpt: 4.2 s from 23.1 s in, "Four score and seven years ago, our fathers brought forth...", the start of the address after the LibriVox introduction.
- Made with `ffmpeg -ss 23.1 -t 4.2 -i gettysburg_address.mp3 -ac 1 -ar 16000 -c:a pcm_s16le -map_metadata -1 -fflags +bitexact -flags:a +bitexact speech.wav`: 16 kHz, mono, 16-bit PCM, 134,444 bytes (SHA-256 `23a51ec92a5251785818fea4b540f24cb5d2cacefa62ab2b172c4000143c8d4d`). `testing::speech()` upsamples it to 48 kHz and scales its peak to -6 dBFS.

The noise the tests mix in (pink noise and a 120 Hz hum) is generated in code by `testing::noise`.
