use std::time::Duration;

use windows::Win32::Foundation::VARIANT_BOOL;
use windows::Win32::Media::MediaFoundation::{IMFAttributes, IMFSample};
use windows::Win32::System::Variant::{
	VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_UI4,
};
use windows::core::GUID;

use super::mf;

const HUNDRED_NANOS: u128 = 100;

pub fn set_size(
	attributes: &IMFAttributes,
	key: &GUID,
	width: u32,
	height: u32,
) -> windows::core::Result<()> {
	unsafe { attributes.SetUINT64(key, (u64::from(width) << 32) | u64::from(height)) }
}

fn variant(kind: VARENUM, value: VARIANT_0_0_0) -> VARIANT {
	VARIANT {
		Anonymous: VARIANT_0 {
			Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
				vt: kind,
				wReserved1: 0,
				wReserved2: 0,
				wReserved3: 0,
				Anonymous: value,
			}),
		},
	}
}

pub fn variant_u32(value: u32) -> VARIANT {
	variant(VT_UI4, VARIANT_0_0_0 { ulVal: value })
}

pub fn variant_bool(value: bool) -> VARIANT {
	variant(
		VT_BOOL,
		VARIANT_0_0_0 {
			boolVal: VARIANT_BOOL(if value { -1 } else { 0 }),
		},
	)
}

pub fn frame_duration(fps: u32) -> Duration {
	Duration::from_secs(1) / fps.max(1)
}

pub fn timed_sample(
	data: &[u8],
	timestamp: Duration,
	duration: Duration,
) -> windows::core::Result<IMFSample> {
	let sample = mf::sample_from(data, timestamp)?;
	let duration = i64::try_from(duration.as_nanos() / HUNDRED_NANOS).unwrap_or(i64::MAX);
	unsafe { sample.SetSampleDuration(duration)? };
	Ok(sample)
}
