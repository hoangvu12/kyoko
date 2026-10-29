use std::fs::File;
use std::io::BufWriter;
use std::time::{Duration, Instant};

use video::{Picture, ScreenKind, VideoError, VideoQuality, open_screen, screens};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

const LIMIT: Duration = Duration::from_secs(120);

fn owner(window: u64) -> u32 {
	let mut process = 0u32;
	unsafe { GetWindowThreadProcessId(HWND(window as usize as *mut _), Some(&mut process)) };
	process
}

fn save(picture: &Picture, path: &str) {
	let file = BufWriter::new(File::create(path).expect("the picture file opens"));
	let mut encoder = png::Encoder::new(file, picture.width, picture.height);
	encoder.set_color(png::ColorType::Rgba);
	encoder.set_depth(png::BitDepth::Eight);
	let mut writer = encoder.write_header().expect("the header is written");
	writer
		.write_image_data(&picture.rgba)
		.expect("the picture is written");
}

fn main() {
	let mut arguments = std::env::args().skip(1);
	let process: u32 = arguments
		.next()
		.and_then(|value| value.parse().ok())
		.expect("the process id whose window to capture");
	let picture_path = arguments.next().expect("where to save the first frame");
	let target = screens()
		.expect("the screens and windows are listed")
		.into_iter()
		.find(|target| target.kind == ScreenKind::Window && owner(target.id) == process)
		.expect("that process has a shareable window");
	println!("capturing a {}x{} window", target.width, target.height);
	let mut source = open_screen(&target, VideoQuality::default()).expect("the capture starts");
	let started = Instant::now();
	let mut frames = 0u32;
	while started.elapsed() < LIMIT {
		match source.next_frame() {
			Ok(frame) => {
				if frames == 0 {
					println!("first frame {}x{}", frame.width, frame.height);
					let picture =
						Picture::from_nv12(&frame, frame.width, frame.height).expect("the frame converts");
					save(&picture, &picture_path);
				}
				frames += 1;
				if frames.is_multiple_of(30) {
					println!("{frames} frames after {:?}", started.elapsed());
				}
			}
			Err(VideoError::Ended) => {
				println!("the window closed: capture ended after {frames} frames");
				return;
			}
			Err(error) => panic!("capture failed: {error}"),
		}
	}
	println!("stopped after {frames} frames without the window closing");
}
