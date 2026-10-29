use std::fs::File;
use std::io;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::ptr::{addr_of_mut, null_mut};

use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT, SetSecurityInfo};
use windows::Win32::Security::{
	ACCESS_ALLOWED_ACE, ACL, ACL_REVISION, AddAccessAllowedAce, DACL_SECURITY_INFORMATION, EqualSid, GetAce,
	GetLengthSid, GetSecurityDescriptorControl, GetTokenInformation, InitializeAcl,
	PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SE_DACL_PROTECTED, TOKEN_QUERY,
	TOKEN_USER, TokenUser,
};
use windows::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;

pub(crate) fn restrict_to_current_user(file: &File) -> io::Result<()> {
	let user = CurrentUser::query()?;
	let sid = user.sid();
	let acl_len = size_of::<ACL>() + size_of::<ACCESS_ALLOWED_ACE>() + unsafe { GetLengthSid(sid) } as usize;
	let mut buffer = vec![0u64; acl_len.div_ceil(size_of::<u64>())];
	let acl = buffer.as_mut_ptr().cast::<ACL>();
	unsafe {
		InitializeAcl(acl, acl_len as u32, ACL_REVISION)?;
		AddAccessAllowedAce(acl, ACL_REVISION, FILE_ALL_ACCESS.0, sid)?;
	}
	let status = unsafe {
		SetSecurityInfo(
			HANDLE(file.as_raw_handle()),
			SE_FILE_OBJECT,
			DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
			None,
			None,
			Some(acl),
			None,
		)
	};
	status.ok().map_err(io::Error::from)
}

pub fn owner_only(path: &Path) -> io::Result<bool> {
	let file = File::open(path)?;
	let user = CurrentUser::query()?;
	let mut dacl: *mut ACL = null_mut();
	let mut descriptor = PSECURITY_DESCRIPTOR::default();
	let status = unsafe {
		GetSecurityInfo(
			HANDLE(file.as_raw_handle()),
			SE_FILE_OBJECT,
			DACL_SECURITY_INFORMATION,
			None,
			None,
			Some(&mut dacl),
			None,
			Some(&mut descriptor),
		)
	};
	status.ok().map_err(io::Error::from)?;
	let only = protected(descriptor) && single_owner_ace(dacl, user.sid());
	unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
	Ok(only)
}

fn protected(descriptor: PSECURITY_DESCRIPTOR) -> bool {
	let mut control = 0u16;
	let mut revision = 0u32;
	let read = unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) };
	read.is_ok() && control & SE_DACL_PROTECTED.0 != 0
}

fn single_owner_ace(dacl: *mut ACL, sid: PSID) -> bool {
	if dacl.is_null() || unsafe { (*dacl).AceCount } != 1 {
		return false;
	}
	let mut ace = null_mut();
	if unsafe { GetAce(dacl, 0, &mut ace) }.is_err() {
		return false;
	}
	let ace = ace.cast::<ACCESS_ALLOWED_ACE>();
	unsafe {
		(*ace).Header.AceType == ACCESS_ALLOWED_ACE_TYPE
			&& (*ace).Mask == FILE_ALL_ACCESS.0
			&& EqualSid(PSID(addr_of_mut!((*ace).SidStart).cast()), sid).is_ok()
	}
}

struct CurrentUser(Vec<u64>);

impl CurrentUser {
	fn query() -> io::Result<Self> {
		let mut token = HANDLE::default();
		unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)? };
		let mut needed = 0;
		let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut needed) };
		let mut buffer = vec![0u64; (needed as usize).div_ceil(size_of::<u64>())];
		let read = unsafe {
			GetTokenInformation(
				token,
				TokenUser,
				Some(buffer.as_mut_ptr().cast()),
				needed,
				&mut needed,
			)
		};
		let _ = unsafe { CloseHandle(token) };
		read?;
		Ok(Self(buffer))
	}

	fn sid(&self) -> PSID {
		unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
	}
}
