use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use image::RgbaImage;

mod browser;
mod compose;
mod font;
mod mockup;
mod report;
mod screens;
mod window;

use screens::ScreenEntry;
use window::Scale;

const MOCK_PORT: u16 = 8485;
const MOCK_START_LIMIT: Duration = Duration::from_secs(15);
const READY_LIMIT: Duration = Duration::from_secs(90);
const CLOSE_GRACE: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(100);
const UNKNOWN_SCREEN: i32 = 3;
const DEFAULT_ANCHOR: &str = "2026-10-01T12:00:00Z";
const REFERENCE_DIR: &str = "oxidcord-ref";
const MOCKUP_ONLY: &[&str] = &[
	"login-qr-scanned",
	"login-qr-expired",
	"login-web",
	"login-email",
	"login-2fa",
	"login-captcha",
	"login-unsaved",
];
const PARITY_LIST: &str = "docs/parity.md";
const MOCKUP: &str = "mockup";

#[derive(clap::Args)]
pub struct Args {
	#[command(subcommand)]
	command: ParityCommand,
}

#[derive(clap::Subcommand)]
enum ParityCommand {
	Capture(CaptureArgs),
	Mockup(MockupArgs),
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
	#[command(flatten)]
	run: RunArgs,
}

#[derive(clap::Args)]
struct MockupArgs {
	#[arg(
		long,
		value_delimiter = ',',
		help = "Screens to compare with their mockups, every mapped screen by default"
	)]
	screens: Vec<String>,
	#[arg(
		long,
		value_name = "PATH",
		help = "Chrome or Edge executable, found automatically by default"
	)]
	browser: Option<PathBuf>,
	#[command(flatten)]
	run: RunArgs,
}

#[derive(clap::Args)]
struct RunArgs {
	#[arg(long, default_value_t = 1, help = "Seed for the mock server fixtures")]
	seed: u64,
	#[arg(long, default_value = DEFAULT_ANCHOR, help = "Fixture date for the mock server")]
	anchor: String,
	#[arg(long, default_value_t = MOCK_PORT, help = "Loopback port for the mock server")]
	port: u16,
	#[arg(
		long,
		value_name = "DIR",
		help = "Output directory, target/parity or target/parity-mockup by default"
	)]
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

pub struct Side {
	pub shot: Shot,
	pub secs: f64,
}

impl Side {
	fn skipped() -> Side {
		Side {
			shot: Shot::Skipped,
			secs: 0.,
		}
	}

	fn timed(capture: impl FnOnce() -> Result<Shot>) -> Result<Side> {
		let started = Instant::now();
		let shot = capture()?;
		Ok(Side {
			shot,
			secs: started.elapsed().as_secs_f64(),
		})
	}
}

pub struct Outcome {
	pub screen: ScreenEntry,
	pub reference: Side,
	pub kyoko: Side,
	pub differing: Option<f64>,
}

impl Outcome {
	fn failed(&self) -> bool {
		matches!(self.reference.shot, Shot::Failed(_)) || matches!(self.kyoko.shot, Shot::Failed(_))
	}

	fn print(&self, reference: &str) {
		println!(
			"{:<32} {reference} {:<14} {:>5.1}s   kyoko {:<14} {:>5.1}s{}",
			self.screen.name,
			self.reference.shot.label(),
			self.reference.secs,
			self.kyoko.shot.label(),
			self.kyoko.secs,
			self.differing
				.map(|share| format!("   {:.1}% differ", share * 100.))
				.unwrap_or_default()
		);
	}
}

struct Paths {
	root: PathBuf,
	out: PathBuf,
	kyoko: PathBuf,
	mockcord: PathBuf,
	oxidcord: PathBuf,
}

impl Paths {
	fn new(root: PathBuf, run: &RunArgs, oxidcord: Option<PathBuf>, out: &str) -> Paths {
		let target = target_dir(&root);
		Paths {
			out: run.out.clone().unwrap_or_else(|| target.join(out)),
			kyoko: target.join("debug").join("kyoko.exe"),
			mockcord: target.join("debug").join("mockcord.exe"),
			oxidcord: oxidcord.unwrap_or_else(|| {
				root.parent()
					.unwrap_or(&root)
					.join(REFERENCE_DIR)
					.join("target")
					.join("debug")
					.join("oxidcord.exe")
			}),
			root,
		}
	}
}

pub fn run(args: Args) -> Result<()> {
	match args.command {
		ParityCommand::List => {
			for screen in screens::load(&workspace_root().join(PARITY_LIST))? {
				println!(
					"{:<32} ticket {:<4} {}",
					screen.name,
					screen.ticket,
					mockup::page_for(&screen.name)
						.map(|page| format!("mockup {page}.html"))
						.unwrap_or_default()
				);
			}
			Ok(())
		}
		ParityCommand::Capture(args) => capture(args),
		ParityCommand::Mockup(args) => compare_mockups(args),
	}
}

fn capture(args: CaptureArgs) -> Result<()> {
	let root = workspace_root();
	let listed = screens::load(&root.join(PARITY_LIST))?;
	let chosen = screens::select(&listed, &args.screens)?;
	if !args.run.no_build {
		cargo_build(&root)?;
	}
	let paths = Paths::new(root, &args.run, args.oxidcord.clone(), "parity");
	if args.apps.contains(&App::Kyoko) {
		require(&paths.kyoko, "")?;
	}
	require(&paths.mockcord, "")?;
	if args.apps.contains(&App::Oxidcord) {
		require(&paths.oxidcord, "; run tools/parity/oxidcord-ref.ps1 first")?;
	}
	window::init_dpi_awareness();
	fs::create_dir_all(&paths.out).with_context(|| format!("create {}", paths.out.display()))?;
	let started = Instant::now();
	let mut outcomes = Vec::new();
	for screen in chosen {
		let outcome = capture_screen(&args, &paths, screen)?;
		outcome.print(App::Oxidcord.name());
		outcomes.push(outcome);
	}
	finish(&paths.out, App::Oxidcord.name(), &outcomes, started, &args.run)
}

fn capture_screen(args: &CaptureArgs, paths: &Paths, screen: ScreenEntry) -> Result<Outcome> {
	let dir = fresh_dir(&paths.out.join(&screen.name))?;
	let side = |app: App| -> Result<Side> {
		let unmatched = app == App::Oxidcord && MOCKUP_ONLY.contains(&screen.name.as_str());
		if !args.apps.contains(&app) || unmatched {
			return Ok(Side::skipped());
		}
		Side::timed(|| capture_app(&args.run, paths, &screen.name, app, &dir, Scale::Standard))
	};
	let oxidcord = side(App::Oxidcord)?;
	let kyoko = side(App::Kyoko)?;
	compose_screen(&dir, App::Oxidcord.name(), screen, oxidcord, kyoko)
}

fn compare_mockups(args: MockupArgs) -> Result<()> {
	let root = workspace_root();
	let listed = screens::load(&root.join(PARITY_LIST))?;
	let pairings = mockup::select(&root, &listed, &args.screens)?;
	let browser_exe = browser::find(args.browser.as_deref())?;
	if !args.run.no_build {
		cargo_build(&root)?;
	}
	let paths = Paths::new(root, &args.run, None, "parity-mockup");
	require(&paths.kyoko, "")?;
	require(&paths.mockcord, "")?;
	window::init_dpi_awareness();
	fs::create_dir_all(&paths.out).with_context(|| format!("create {}", paths.out.display()))?;
	println!("rendering mockups with {}", browser_exe.display());
	let mut browser = browser::Browser::start(&browser_exe, &paths.out.join("browser.log"))?;
	let started = Instant::now();
	let mut outcomes = Vec::new();
	for pairing in pairings {
		match compare_mockup(&args.run, &paths, &mut browser, pairing) {
			Ok(outcome) => {
				outcome.print(MOCKUP);
				outcomes.push(outcome);
			}
			Err(error) => {
				browser.stop();
				return Err(error);
			}
		}
	}
	browser.stop();
	finish(&paths.out, MOCKUP, &outcomes, started, &args.run)
}

fn compare_mockup(
	run: &RunArgs,
	paths: &Paths,
	browser: &mut browser::Browser,
	pairing: mockup::Pairing,
) -> Result<Outcome> {
	let dir = fresh_dir(&paths.out.join(&pairing.screen.name))?;
	let kyoko =
		Side::timed(|| capture_app(run, paths, &pairing.screen.name, App::Kyoko, &dir, Scale::Window))?;
	let scale = match &kyoko.shot {
		Shot::Captured(image) => f64::from(image.width()) / f64::from(compose::WIDTH),
		_ => window::system_scale(),
	};
	let reference = Side::timed(|| {
		Ok(match browser.render(&pairing.page, scale) {
			Ok(image) => Shot::Captured(image),
			Err(error) => Shot::Failed(format!("{error:#}")),
		})
	})?;
	if let Shot::Failed(reason) = &reference.shot {
		fs::write(dir.join("mockup.log"), reason).context("write the mockup log")?;
	}
	compose_screen(&dir, MOCKUP, pairing.screen, reference, kyoko)
}

fn fresh_dir(dir: &Path) -> Result<PathBuf> {
	let _ = fs::remove_dir_all(dir);
	fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
	Ok(dir.to_path_buf())
}

fn compose_screen(
	dir: &Path,
	reference_name: &str,
	screen: ScreenEntry,
	reference: Side,
	kyoko: Side,
) -> Result<Outcome> {
	let reference_image = compose::shot_image(reference_name, &screen.name, &reference.shot);
	let kyoko_image = compose::shot_image(App::Kyoko.name(), &screen.name, &kyoko.shot);
	save(&reference_image, &dir.join(format!("{reference_name}.png")))?;
	save(&kyoko_image, &dir.join("kyoko.png"))?;
	save(
		&compose::pair(reference_name, &reference_image, &kyoko_image),
		&dir.join("pair.png"),
	)?;
	let differing = match (&reference.shot, &kyoko.shot) {
		(Shot::Captured(left), Shot::Captured(right)) => {
			let differing = compose::diff(left, right)
				.map(|(image, share)| save(&image, &dir.join("diff.png")).map(|()| share))
				.transpose()?;
			if differing.is_none() {
				eprintln!(
					"{}: {reference_name} is {}x{} but kyoko {}x{}, so there is no diff",
					screen.name,
					left.width(),
					left.height(),
					right.width(),
					right.height()
				);
			}
			differing
		}
		_ => None,
	};
	Ok(Outcome {
		screen,
		reference,
		kyoko,
		differing,
	})
}

fn finish(out: &Path, reference: &str, outcomes: &[Outcome], started: Instant, run: &RunArgs) -> Result<()> {
	let elapsed = started.elapsed();
	report::write(out, reference, outcomes, elapsed, run.seed, &run.anchor)?;
	println!(
		"captured {} screens in {:.0}s, review {}",
		outcomes.len(),
		elapsed.as_secs_f64(),
		out.join("index.html").display()
	);
	let failed = outcomes.iter().filter(|outcome| outcome.failed()).count();
	if failed > 0 {
		bail!("{failed} screens had a failed capture, see the logs next to their images");
	}
	Ok(())
}

fn require(exe: &Path, hint: &str) -> Result<()> {
	if !exe.exists() {
		bail!("{} is missing{hint}", exe.display());
	}
	Ok(())
}

fn capture_app(
	args: &RunArgs,
	paths: &Paths,
	screen: &str,
	app: App,
	dir: &Path,
	scale: Scale,
) -> Result<Shot> {
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
		Waited::Ready => match window::capture(child.id(), app, scale) {
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
	fn start(paths: &Paths, args: &RunArgs, log: &Path) -> Result<Mockcord> {
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
