use std::io;
use std::os::windows::fs::FileExt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use video::{ByteSource, FileReader, Packet, Picture};

struct OnDisk(std::fs::File);

impl ByteSource for OnDisk {
	fn length(&self) -> io::Result<Option<u64>> {
		Ok(Some(self.0.metadata()?.len()))
	}

	fn read_at(&self, position: u64, buffer: &mut [u8]) -> io::Result<usize> {
		self.0.seek_read(buffer, position)
	}
}

fn main() {
	let path = std::env::args().nth(1).expect("a clip path");
	let file = std::fs::File::open(&path).expect("the clip opens");
	let started = Instant::now();
	let mut reader = FileReader::open(Arc::new(OnDisk(file)), &path).expect("the reader opens");
	println!("tracks {:?} in {:?}", reader.tracks(), started.elapsed());
	let (mut frames, mut sound, mut last) = (0, 0, Duration::ZERO);
	loop {
		match reader.packet().expect("a packet") {
			Packet::Video(frame) => {
				frames += 1;
				last = frame.timestamp;
				let _ = Picture::from_nv12(&frame, 160, 90).expect("a picture");
			}
			Packet::Audio { samples, .. } => sound += samples.len(),
			Packet::End => break,
		}
	}
	println!(
		"{frames} frames, last at {last:?}, {sound} samples, in {:?}",
		started.elapsed()
	);
	reader.seek(Duration::from_secs(1)).expect("seek");
	while let Ok(packet) = reader.packet() {
		if let Packet::Video(frame) = packet {
			println!("after seeking to 1 s the first frame is at {:?}", frame.timestamp);
			break;
		}
	}
}
