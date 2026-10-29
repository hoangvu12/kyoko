use std::mem::ManuallyDrop;
use std::sync::mpsc;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
	AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
	AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK, AUDIOCLIENT_ACTIVATION_PARAMS,
	AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
	AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation,
	IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
	IAudioCaptureClient, IAudioClient, PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE,
	PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
	WAVE_FORMAT_PCM, WAVEFORMATEX,
};
use windows::Win32::System::Com::StructuredStorage::{
	PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
};
use windows::Win32::System::Com::{BLOB, COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::System::Variant::VT_BLOB;
use windows::core::{HRESULT, IUnknown, Interface, Ref, implement};

use super::{SoundSource, SoundTarget};
use crate::codec::SAMPLE_RATE;
use crate::error::{Result, VoiceError};

const CHANNELS: u16 = 2;
const BITS: u16 = 16;
const BUFFER_100NS: i64 = 2_000_000;
const ACTIVATE_TIMEOUT: Duration = Duration::from_secs(5);
const WAIT_MS: u32 = 100;

fn failure(error: windows::core::Error) -> VoiceError {
	VoiceError::Audio(error.message())
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct Activated(mpsc::Sender<()>);

impl IActivateAudioInterfaceCompletionHandler_Impl for Activated_Impl {
	fn ActivateCompleted(
		&self,
		_operation: Ref<IActivateAudioInterfaceAsyncOperation>,
	) -> windows::core::Result<()> {
		let _ = self.0.send(());
		Ok(())
	}
}

pub(super) struct ProcessLoopback {
	client: IAudioClient,
	capture: IAudioCaptureClient,
	event: HANDLE,
}

unsafe impl Send for ProcessLoopback {}

fn prepare_thread() {
	let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

fn format() -> WAVEFORMATEX {
	let block = CHANNELS * BITS / 8;
	WAVEFORMATEX {
		wFormatTag: WAVE_FORMAT_PCM as u16,
		nChannels: CHANNELS,
		nSamplesPerSec: SAMPLE_RATE,
		nAvgBytesPerSec: SAMPLE_RATE * u32::from(block),
		nBlockAlign: block,
		wBitsPerSample: BITS,
		cbSize: 0,
	}
}

fn activate(target: SoundTarget) -> Result<IAudioClient> {
	let (process, mode) = match target {
		SoundTarget::Everything { except } => (except, PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE),
		SoundTarget::App { process } => (process, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE),
	};
	let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
		ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
		Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
			ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
				TargetProcessId: process,
				ProcessLoopbackMode: mode,
			},
		},
	};
	let blob = ManuallyDrop::new(PROPVARIANT {
		Anonymous: PROPVARIANT_0 {
			Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
				vt: VT_BLOB,
				wReserved1: 0,
				wReserved2: 0,
				wReserved3: 0,
				Anonymous: PROPVARIANT_0_0_0 {
					blob: BLOB {
						cbSize: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
						pBlobData: (&raw mut params).cast(),
					},
				},
			}),
		},
	});
	let (done, completed) = mpsc::channel();
	let handler: IActivateAudioInterfaceCompletionHandler = Activated(done).into();
	let operation = unsafe {
		ActivateAudioInterfaceAsync(
			VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
			&IAudioClient::IID,
			Some(&raw const *blob),
			&handler,
		)
	}
	.map_err(failure)?;
	completed
		.recv_timeout(ACTIVATE_TIMEOUT)
		.map_err(|_| VoiceError::Audio("sound capture did not activate".to_owned()))?;
	let mut result = HRESULT(0);
	let mut activated: Option<IUnknown> = None;
	unsafe { operation.GetActivateResult(&mut result, &mut activated) }.map_err(failure)?;
	result.ok().map_err(failure)?;
	activated
		.ok_or_else(|| VoiceError::Audio("sound capture gave no client".to_owned()))?
		.cast::<IAudioClient>()
		.map_err(failure)
}

impl ProcessLoopback {
	pub(super) fn open(target: SoundTarget) -> Result<Self> {
		prepare_thread();
		let client = activate(target)?;
		let format = format();
		unsafe {
			client
				.Initialize(
					AUDCLNT_SHAREMODE_SHARED,
					AUDCLNT_STREAMFLAGS_LOOPBACK
						| AUDCLNT_STREAMFLAGS_EVENTCALLBACK
						| AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
					BUFFER_100NS,
					0,
					&format,
					None,
				)
				.map_err(failure)?;
			let capture: IAudioCaptureClient = client.GetService().map_err(failure)?;
			let event = CreateEventW(None, false, false, None).map_err(failure)?;
			if let Err(error) = client.SetEventHandle(event).and_then(|()| client.Start()) {
				let _ = CloseHandle(event);
				return Err(failure(error));
			}
			Ok(Self {
				client,
				capture,
				event,
			})
		}
	}
}

impl SoundSource for ProcessLoopback {
	fn read(&mut self, stereo: &mut Vec<f32>) -> Result<()> {
		prepare_thread();
		if unsafe { WaitForSingleObject(self.event, WAIT_MS) } != WAIT_OBJECT_0 {
			return Ok(());
		}
		unsafe {
			while self.capture.GetNextPacketSize().map_err(failure)? > 0 {
				let mut data = std::ptr::null_mut();
				let mut frames = 0u32;
				let mut flags = 0u32;
				self.capture
					.GetBuffer(&mut data, &mut frames, &mut flags, None, None)
					.map_err(failure)?;
				let samples = frames as usize * usize::from(CHANNELS);
				if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
					stereo.extend(std::iter::repeat_n(0.0, samples));
				} else {
					let pcm = std::slice::from_raw_parts(data.cast::<i16>(), samples);
					stereo.extend(pcm.iter().map(|sample| f32::from(*sample) / f32::from(i16::MAX)));
				}
				self.capture.ReleaseBuffer(frames).map_err(failure)?;
			}
		}
		Ok(())
	}
}

impl Drop for ProcessLoopback {
	fn drop(&mut self) {
		unsafe {
			let _ = self.client.Stop();
			let _ = CloseHandle(self.event);
		}
	}
}
