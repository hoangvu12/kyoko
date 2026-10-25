use auto_launch::{AutoLaunch, AutoLaunchBuilder};

use crate::{PlatformError, Result};

pub fn set_enabled(enabled: bool, args: &[&str]) -> Result<()> {
	let launcher = launcher(args)?;
	match (enabled, launcher.is_enabled()?) {
		(true, _) => launcher.enable()?,
		(false, true) => launcher.disable()?,
		_ => {}
	}
	Ok(())
}

pub fn is_enabled() -> Result<bool> {
	Ok(launcher(&[])?.is_enabled()?)
}

fn launcher(args: &[&str]) -> Result<AutoLaunch> {
	let exe = std::env::current_exe()?;
	let name = exe
		.file_stem()
		.and_then(|stem| stem.to_str())
		.ok_or_else(|| PlatformError::Os("executable name is not valid unicode".to_owned()))?;
	let path = exe
		.to_str()
		.ok_or_else(|| PlatformError::Os("executable path is not valid unicode".to_owned()))?;
	Ok(AutoLaunchBuilder::new()
		.set_app_name(name)
		.set_app_path(&format!("\"{path}\""))
		.set_args(args)
		.build()?)
}
