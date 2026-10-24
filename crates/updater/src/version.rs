#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionInfo {
	pub version: &'static str,
	pub git_sha: Option<&'static str>,
	pub build_date: &'static str,
}

pub fn version_info() -> VersionInfo {
	VersionInfo {
		version: env!("CARGO_PKG_VERSION"),
		git_sha: option_env!("KYOKO_GIT_SHA"),
		build_date: env!("KYOKO_BUILD_DATE"),
	}
}
