use zgx_desktop::FontData;

const BUNDLED: [&[u8]; 4] = [
	include_bytes!("../fonts/inter/Inter-Regular.ttf"),
	include_bytes!("../fonts/inter/Inter-Medium.ttf"),
	include_bytes!("../fonts/inter/Inter-SemiBold.ttf"),
	include_bytes!("../fonts/inter/Inter-Bold.ttf"),
];

const STYLES: &[&str] = &[
	"regular",
	"normal",
	"book",
	"thin",
	"hairline",
	"extralight",
	"ultralight",
	"light",
	"semilight",
	"medium",
	"semibold",
	"demibold",
	"bold",
	"extrabold",
	"ultrabold",
	"black",
	"heavy",
	"italic",
	"oblique",
	"condensed",
	"variable",
];

const GENERIC: &[&str] = &["sans-serif", "serif", "monospace", "system-ui"];

pub fn pick(families: &[String], installed: &[String]) -> String {
	families
		.iter()
		.find(|family| {
			GENERIC.contains(&family.to_ascii_lowercase().as_str())
				|| installed.iter().any(|name| name.eq_ignore_ascii_case(family))
		})
		.or_else(|| families.last())
		.cloned()
		.unwrap_or_else(|| "sans-serif".to_owned())
}

pub fn bundled() -> Vec<FontData> {
	BUNDLED
		.into_iter()
		.filter_map(|bytes| match FontData::new(bytes) {
			Ok(font) => Some(font),
			Err(error) => {
				tracing::warn!(%error, "a bundled font could not be read");
				None
			}
		})
		.collect()
}

pub fn installed() -> Vec<String> {
	let mut families: Vec<String> = registered_faces()
		.iter()
		.filter_map(|face| family_of(face))
		.chain(bundled().iter().flat_map(|font| font.families().to_vec()))
		.collect();
	families.sort_by_key(|family| family.to_ascii_lowercase());
	families.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
	families
}

fn family_of(face: &str) -> Option<String> {
	let name = face
		.rsplit_once(" (")
		.map_or(face, |(name, _)| name)
		.split(" & ")
		.next()?
		.trim();
	let mut words: Vec<&str> = name.split_whitespace().collect();
	while words.len() > 1
		&& words
			.last()
			.is_some_and(|word| STYLES.contains(&word.to_ascii_lowercase().as_str()))
	{
		words.pop();
	}
	(!words.is_empty()).then(|| words.join(" "))
}

#[cfg(windows)]
fn registered_faces() -> Vec<String> {
	use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

	const KEY: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
	[HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER]
		.into_iter()
		.flat_map(|root: HKEY| registry_value_names(root, KEY))
		.collect()
}

#[cfg(windows)]
fn registry_value_names(root: windows::Win32::System::Registry::HKEY, path: &str) -> Vec<String> {
	use windows::Win32::Foundation::ERROR_SUCCESS;
	use windows::Win32::System::Registry::{HKEY, KEY_READ, RegCloseKey, RegEnumValueW, RegOpenKeyExW};
	use windows::core::{PCWSTR, PWSTR};

	let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
	let mut key = HKEY::default();
	let opened = unsafe { RegOpenKeyExW(root, PCWSTR(wide.as_ptr()), None, KEY_READ, &mut key) };
	if opened != ERROR_SUCCESS {
		return Vec::new();
	}
	let mut names = Vec::new();
	let mut buffer = [0u16; 512];
	for index in 0.. {
		let mut length = buffer.len() as u32;
		let status = unsafe {
			RegEnumValueW(
				key,
				index,
				Some(PWSTR(buffer.as_mut_ptr())),
				&mut length,
				None,
				None,
				None,
				None,
			)
		};
		if status != ERROR_SUCCESS {
			break;
		}
		names.push(String::from_utf16_lossy(&buffer[..length as usize]));
	}
	unsafe {
		let _ = RegCloseKey(key);
	}
	names
}

#[cfg(not(windows))]
fn registered_faces() -> Vec<String> {
	Vec::new()
}
