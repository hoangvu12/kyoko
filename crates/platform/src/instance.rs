use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, GetLastError};
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::core::HSTRING;

pub fn is_running(app_id: &str) -> bool {
	let name = pipe_name(app_id);
	if unsafe { WaitNamedPipeW(&name, 1) }.as_bool() {
		return true;
	}
	let error = unsafe { GetLastError() };
	error != ERROR_FILE_NOT_FOUND
}

fn pipe_name(app_id: &str) -> HSTRING {
	let mut session = 0;
	let _ = unsafe { ProcessIdToSessionId(std::process::id(), &mut session) };
	HSTRING::from(format!(r"\\.\pipe\{app_id}.{session}"))
}
