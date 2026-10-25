use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use image::RgbaImage;

mod compose;
mod font;
mod report;
mod screens;
mod window;

use screens::ScreenEntry;

const MOCK_PORT: u16 = 8485;
const MOCK_START_LIMIT: Duration = Duration::from_secs(15);
const READY_LIMIT: Duration = Duration::from_secs(90);
const CLOSE_GRACE: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(100);
const UNKNOWN_SCREEN: i32 = 3;
const DEFAULT_ANCHOR: &str = "2026-10-01T12:00:00Z";
const REFERENCE_DIR: &str = "oxidcord-ref";
const PARITY_LIST: &str = "docs/parity.md";

#[derive(clap::Args)]
pub struct Args {
	#[command(subcommand)]
	command: ParityCommand,
}

#[derive(clap::Subcommand)]
enum ParityCommand {
	Capture(CaptureArgs),
	List,
}

#[derive(clap::Args)]
struct CaptureArgs {
	#[arg(
		long,
		value_delimiter = ',',
		help = "Screens to capture, all of docs/parity.md by default"
	)]
	screens: Vec<String>,
	#[arg(long, value_delimiter = ',', default_values_t = [App::Oxidcord, App::Kyoko], help = "Apps to capture")]
	apps: Vec<App>,
	#[arg(long, value_name = "PATH", help = "The oxidcord reference executable")]
	oxidcord: Option<PathBuf>,
	#[arg(long, default_value_t = 1, help = "Seed for the mock server fixtures")]
	seed: u64,
	#[arg(long, default_value = DEFAULT_ANCHOR, help = "Fixture date for the mock server")]
	anchor: String,
	#[arg(long, default_value_t = MOCK_PORT, help = "Loopback port for the mock server")]
	port: u16,
	#[arg(long, value_name = "DIR", help = "Output directory, target/parity by default")]
	out: Option<PathBuf>,
	#[arg(
		long,
		help = "Use the kyoko and mockcord executables as they are, without building"
	)]
	no_build: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum App {
	Oxidcord,
	Kyoko,
}

impl App {
	pub fn name(self) -> &'static str {
		match self {
			App::Oxidcord => "oxidcord",
			App::Kyoko => "kyoko",
		}
	}
}

impl std::fmt::Display for App {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str(self.name())
	}
}

pub enum Shot {
	Captured(RgbaImage),
	NotBuilt,
	Skipped,
	Failed(String),
}

impl Shot {
	pub fn label(&self) -> &'static str {
		match self {
			Shot::Captured(_) => "captured",
			Shot::NotBuilt => "not built yet",
			Shot::Skipped => "skipped",
			Shot::Failed(_) => "failed",
		}
	}
}

pub struct Outcome {
	pub screen: ScreenEntry,
	pub oxidcord: Shot,
	pub kyoko: Shot,
	pub oxidcord_secs: f64,
	pub kyoko_secs: f64,
	pub differing: Option<f64>,
}

struct Paths {
	root: PathBuf,
	out: PathBuf,
	kyoko: PathBuf,
	mockcord: PathBuf,
	oxidcord: PathBuf,
}

pub fn run(args: Args) -> Result<()> {
	match args.command {
		ParityCommand::List => {
			for screen in screens::load(&workspace_root().join(PARITY_LIST))? {
				println!("{:<32} ticket {}", screen.name, screen.ticket);
			}
			Ok(())
		}
		ParityCommand::Capture(args) => capture(args),
	}
}

fn capture(args: CaptureArgs) -> Result<()> {
	let root = workspace_root();
	let listed = screens::load(&root.join(PARITY_LIST))?;
	let chosen = screens::select(&listed, &args.screens)?;
	if !args.no_build {
		cargo_build(&root)?;
	}
	let target = target_dir(&root);
	let paths = Paths {
		out: args.out.clone().unwrap_or_else(|| target.join("parity")),
		kyoko: target.join("debug").join("kyoko.exe"),
		mockcord: target.join("debug").join("mockcord.exe"),
		oxidcord: args.oxidcord.clone().unwrap_or_else(|| {
			root.parent()
				.unwrap_or(&root)
				.join(REFERENCE_DIR)
				.join("target")
				.join("debug")
				.join("oxidcord.exe")
		}),
		root,
	};
	for (app, exe) in [
		(Some(App::Kyoko), &paths.kyoko),
		(None, &paths.mockcord),
		(Some(App::Oxidcord), &paths.oxidcord),
	] {
		let needed = app.is_none_or(|app| args.apps.contains(&app));
		if needed && !exe.exists() {
			bail!(
				"{} is missing; for oxidcord run tools/parity/oxidcord-ref.ps1 first",
				exe.display()
			);
		}
	}
	window::init_dpi_awareness();
	fs::create_dir_all(&paths.out).with_context(|| format!("create {}", paths.out.display()))?;
	let started = Instant::now();
	let mut outcomes = Vec::new();
	for screen in chosen {
		let outcome = capture_screen(&args, &paths, screen)?;
		println!(
			"{:<32} oxidcord {:<14} {:>5.1}s   kyoko {:<14} {:>5.1}s{}",
			outcome.screen.name,
			outcome.oxidcord.label(),
			outcome.oxidcord_secs,
			outcome.kyoko.label(),
			outcome.kyoko_secs,
			outcome
				.differing
				.map(|share| format!("   {:.1}% differ", share * 100.))
				.unwrap_or_default()
		);
		outcomes.push(outcome);
	}
	let elapsed = started.elapsed();
	report::write(&paths.out, &outcomes, elapsed, args.seed, &args.anchor)?;
	println!(
		"captured {} screens in {:.0}s, review {}",
		outcomes.len(),
		elapsed.as_secs_f64(),
		paths.out.join("index.html").display()
	);
	let failed = outcomes
		.iter()
		.filter(|outcome| {
			matches!(outcome.oxidcord, Shot::Failed(_)) || matches!(outcome.kyoko, Shot::Failed(_))
		})
		.count();
	if failed > 0 {
		bail!("{failed} screens had a failed capture, see the logs next to their images");
	}
	Ok(())
}

fn capture_screen(args: &CaptureArgs, paths: &Paths, screen: ScreenEntry) -> Result<Outcome> {
	let dir = paths.out.join(&screen.name);
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
	let timed = |app: App| -> Result<(Shot, f64)> {
		if !args.apps.contains(&app) {
			return Ok((Shot::Skipped, 0.));
		}
		let started = Instant::now();
		let shot = capture_app(args, paths, &screen.name, app, &dir)?;
		Ok((shot, started.elapsed().as_secs_f64()))
	};
	let (oxidcord, oxidcord_secs) = timed(App::Oxidcord)?;
	let (kyoko, kyoko_secs) = timed(App::Kyoko)?;
	let oxidcord_image = compose::shot_image(App::Oxidcord, &screen.name, &oxidcord);
	let kyoko_image = compose::shot_image(App::Kyoko, &screen.name, &kyoko);
	save(&oxidcord_image, &dir.join("oxidcord.png"))?;
	save(&kyoko_image, &dir.join("kyoko.png"))?;
	save(
		&compose::pair(&oxidcord_image, &kyoko_image),
		&dir.join("pair.png"),
	)?;
	let differing = match (&oxidcord, &kyoko) {
		(Shot::Captured(left), Shot::Captured(right)) => compose::diff(left, right)
			.map(|(image, share)| save(&image, &dir.join("diff.png")).map(|()| share))
			.transpose()?,
		_ => None,
	};
	Ok(Outcome {
		screen,
		oxidcord,
		kyoko,
		oxidcord_secs,
		kyoko_secs,
		differing,
	})
}

fn capture_app(args: &CaptureArgs, paths: &Paths, screen: &str, app: App, dir: &Path) -> Result<Shot> {
	let mock = Mockcord::start(paths, args, &dir.join(format!("{app}-mockcord.log")))?;
	let data_dir = std::env::temp_dir().join(format!("kyoko-parity-{app}-{}", std::process::id()));
	let _ = fs::remove_dir_all(&data_dir);
	let ready = dir.join(format!("{app}.ready"));
	let _ = fs::remove_file(&ready);
	let origin = format!("http://127.0.0.1:{}/", args.port);
	let (exe, mut arguments) = match app {
		App::Oxidcord => (&paths.oxidcord, vec!["--mock".to_owned()]),
		App::Kyoko => (&paths.kyoko, Vec::new()),
	};
	arguments.extend([
		"--mock-server".to_owned(),
		origin,
		"--capture-screen".to_owned(),
		screen.to_owned(),
		"--capture-ready".to_owned(),
		ready.display().to_string(),
		"--data-dir".to_owned(),
		data_dir.display().to_string(),
	]);
	let log = fs::File::create(dir.join(format!("{app}.log"))).context("create the app log")?;
	let mut child = Command::new(exe)
		.args(&arguments)
		.current_dir(&paths.root)
		.stdin(Stdio::null())
		.stdout(log.try_clone().context("share the app log")?)
		.stderr(log)
		.spawn()
		.with_context(|| format!("start {}", exe.display()))?;
	let shot = match wait_ready(&mut child, &ready, app)? {
		Waited::Ready => match window::capture(child.id(), app) {
			Ok(image) => Shot::Captured(image),
			Err(error) => Shot::Failed(format!("{error:#}")),
		},
		Waited::Exited(status) if app == App::Kyoko && status.code() == Some(UNKNOWN_SCREEN) => {
			Shot::NotBuilt
		}
		Waited::Exited(status) => Shot::Failed(format!("{app} exited early with {status}")),
		Waited::TimedOut => Shot::Failed(format!("{app} did not signal ready in time")),
	};
	close(&mut child);
	mock.stop();
	let _ = fs::remove_file(&ready);
	remove_eventually(&data_dir);
	Ok(shot)
}

enum Waited {
	Ready,
	Exited(ExitStatus),
	TimedOut,
}

fn wait_ready(child: &mut Child, ready: &Path, app: App) -> Result<Waited> {
	let deadline = Instant::now() + READY_LIMIT;
	loop {
		if ready.exists() {
			return Ok(Waited::Ready);
		}
		if let Some(status) = child.try_wait().with_context(|| format!("watch {app}"))? {
			return Ok(Waited::Exited(status));
		}
		if Instant::now() >= deadline {
			return Ok(Waited::TimedOut);
		}
		std::thread::sleep(POLL);
	}
}

fn close(child: &mut Child) {
	window::request_close(child.id());
	let deadline = Instant::now() + CLOSE_GRACE;
	while Instant::now() < deadline {
		if matches!(child.try_wait(), Ok(Some(_))) {
			return;
		}
		std::thread::sleep(POLL);
	}
	let _ = child.kill();
	let _ = child.wait();
}

fn remove_eventually(dir: &Path) {
	for _ in 0..20 {
		if fs::remove_dir_all(dir).is_ok() || !dir.exists() {
			return;
		}
		std::thread::sleep(POLL);
	}
}

struct Mockcord(Child);

impl Mockcord {
	fn start(paths: &Paths, args: &CaptureArgs, log: &Path) -> Result<Mockcord> {
		let log = fs::File::create(log).context("create the mockcord log")?;
		let child = Command::new(&paths.mockcord)
			.args([
				"--seed",
				&args.seed.to_string(),
				"--port",
				&args.port.to_string(),
				"--anchor",
				&args.anchor,
			])
			.stdin(Stdio::null())
			.stdout(log.try_clone().context("share the mockcord log")?)
			.stderr(log)
			.spawn()
			.context("start mockcord")?;
		let mut mock = Mockcord(child);
		let deadline = Instant::now() + MOCK_START_LIMIT;
		while TcpStream::connect(("127.0.0.1", args.port)).is_err() {
			if let Ok(Some(status)) = mock.0.try_wait() {
				bail!("mockcord exited with {status}; is port {} free?", args.port);
			}
			if Instant::now() >= deadline {
				mock.stop();
				bail!("mockcord did not listen on port {} in time", args.port);
			}
			std::thread::sleep(POLL);
		}
		Ok(mock)
	}

	fn stop(mut self) {
		let _ = self.0.kill();
		let _ = self.0.wait();
	}
}

fn save(image: &RgbaImage, path: &Path) -> Result<()> {
	image
		.save(path)
		.with_context(|| format!("write {}", path.display()))
}

fn cargo_build(root: &Path) -> Result<()> {
	let program = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
	let status = Command::new(&program)
		.args(["build", "-p", "kyoko", "-p", "mockcord"])
		.current_dir(root)
		.status()
		.with_context(|| format!("could not start {program}"))?;
	if !status.success() {
		bail!("building kyoko and mockcord failed");
	}
	Ok(())
}

fn workspace_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.ancestors()
		.nth(2)
		.map(Path::to_path_buf)
		.unwrap_or_else(|| PathBuf::from("."))
}

fn target_dir(root: &Path) -> PathBuf {
	std::env::var_os("CARGO_TARGET_DIR")
		.map(PathBuf::from)
		.unwrap_or_else(|| root.join("target"))
}
