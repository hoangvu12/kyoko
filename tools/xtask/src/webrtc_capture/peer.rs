use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use mockcord::hello::{ClientHello, DtlsHelloReader};
use parking_lot::Mutex;
use str0m::change::{SdpAnswer, SdpOffer};
use str0m::config::DtlsVersion;
use str0m::crypto::CryptoProvider;
use str0m::media::{Direction, MediaKind};
use str0m::net::{Protocol, Receive, Transmit};
use str0m::{Candidate, Event, Input, Output, Rtc, RtcConfig};

const LOOPBACK: &str = "127.0.0.1:0";
const DATAGRAM: usize = 2_000;
const IDLE: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Backend {
	Wincrypto,
}

impl Backend {
	pub fn label(self) -> &'static str {
		match self {
			Backend::Wincrypto => "str0m-wincrypto",
		}
	}

	fn provider(self) -> CryptoProvider {
		match self {
			Backend::Wincrypto => str0m_wincrypto::default_provider(),
		}
	}

	fn rtc(self, dtls: Dtls) -> Rtc {
		RtcConfig::new()
			.set_crypto_provider(Arc::new(self.provider()))
			.set_dtls_version(dtls.version())
			.build(Instant::now())
	}
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Dtls {
	#[value(name = "1.2")]
	V12,
	#[value(name = "1.3")]
	V13,
	Auto,
}

impl Dtls {
	pub fn label(self) -> &'static str {
		match self {
			Dtls::V12 => "DTLS 1.2",
			Dtls::V13 => "DTLS 1.3",
			Dtls::Auto => "DTLS auto",
		}
	}

	fn version(self) -> DtlsVersion {
		match self {
			Dtls::V12 => DtlsVersion::Dtls12,
			Dtls::V13 => DtlsVersion::Dtls13,
			Dtls::Auto => DtlsVersion::Auto,
		}
	}
}

#[derive(Default)]
pub struct Observed {
	pub hello: Option<ClientHello>,
	pub reader: DtlsHelloReader,
	pub datagrams: usize,
	pub states: Vec<String>,
	pub connected: bool,
	pub error: Option<String>,
}

pub struct Answerer {
	pub answer: String,
	pub observed: Arc<Mutex<Observed>>,
	stop: Arc<AtomicBool>,
	thread: Option<std::thread::JoinHandle<()>>,
}

impl Answerer {
	pub fn start(backend: Backend, dtls: Dtls, offer: &str) -> Result<Answerer> {
		let offer = SdpOffer::from_sdp_string(offer).map_err(|error| anyhow!("parse the offer: {error}"))?;
		let socket = UdpSocket::bind(LOOPBACK).context("bind a loopback UDP socket")?;
		let local = socket.local_addr()?;
		let mut rtc = backend.rtc(dtls);
		let candidate = Candidate::host(local, "udp").map_err(|error| anyhow!("{error}"))?;
		rtc.add_local_candidate(candidate);
		let answer = rtc
			.sdp_api()
			.accept_offer(offer)
			.map_err(|error| anyhow!("accept the offer: {error}"))?;
		let observed = Arc::new(Mutex::new(Observed::default()));
		let stop = Arc::new(AtomicBool::new(false));
		let thread = std::thread::spawn({
			let observed = observed.clone();
			let stop = stop.clone();
			move || {
				if let Err(error) = serve(rtc, &socket, &observed, &stop) {
					observed.lock().error = Some(format!("{error:#}"));
				}
			}
		});
		Ok(Answerer {
			answer: answer.to_sdp_string(),
			observed,
			stop,
			thread: Some(thread),
		})
	}
}

impl Drop for Answerer {
	fn drop(&mut self) {
		self.stop.store(true, Ordering::Relaxed);
		if let Some(thread) = self.thread.take() {
			let _ = thread.join();
		}
	}
}

fn serve(mut rtc: Rtc, socket: &UdpSocket, observed: &Mutex<Observed>, stop: &AtomicBool) -> Result<()> {
	let local = socket.local_addr()?;
	let mut buffer = vec![0u8; DATAGRAM];
	while !stop.load(Ordering::Relaxed) {
		let deadline = match rtc.poll_output()? {
			Output::Timeout(deadline) => deadline,
			Output::Transmit(transmit) => {
				socket.send_to(&transmit.contents, transmit.destination)?;
				continue;
			}
			Output::Event(event) => {
				note(observed, &event);
				continue;
			}
		};
		let wait = deadline.saturating_duration_since(Instant::now()).min(IDLE);
		if wait.is_zero() {
			rtc.handle_input(Input::Timeout(Instant::now()))?;
			continue;
		}
		socket.set_read_timeout(Some(wait))?;
		match socket.recv_from(&mut buffer) {
			Ok((length, source)) => {
				let datagram = &buffer[..length];
				record(observed, datagram);
				rtc.handle_input(Input::Receive(
					Instant::now(),
					Receive {
						proto: Protocol::Udp,
						source,
						destination: local,
						contents: datagram.try_into()?,
					},
				))?;
			}
			Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
				rtc.handle_input(Input::Timeout(Instant::now()))?;
			}
			Err(error) if error.kind() == ErrorKind::ConnectionReset => {}
			Err(error) => return Err(error.into()),
		}
	}
	Ok(())
}

fn record(observed: &Mutex<Observed>, datagram: &[u8]) {
	let mut observed = observed.lock();
	observed.datagrams += 1;
	if observed.hello.is_some() {
		return;
	}
	match observed.reader.feed(datagram) {
		Ok(Some(hello)) => observed.hello = Some(hello),
		Ok(None) => {}
		Err(error) => observed.error = Some(format!("unreadable DTLS ClientHello: {error}")),
	}
}

fn note(observed: &Mutex<Observed>, event: &Event) {
	let mut observed = observed.lock();
	match event {
		Event::IceConnectionStateChange(state) => observed.states.push(format!("ice {state:?}")),
		Event::Connected => {
			observed.connected = true;
			observed.states.push("dtls connected".to_owned());
		}
		_ => {}
	}
}

pub struct Loopback {
	pub hello: ClientHello,
	pub hello_bytes: usize,
	pub hello_datagrams: usize,
	pub connected: bool,
	pub offer: String,
}

pub fn loopback(backend: Backend, dtls: Dtls, wait: Duration) -> Result<Loopback> {
	let offerer_address: SocketAddr = "127.0.0.1:50000".parse()?;
	let answerer_address: SocketAddr = "127.0.0.1:50001".parse()?;
	let mut offerer = backend.rtc(dtls);
	let mut answerer = backend.rtc(dtls);
	offerer.add_local_candidate(Candidate::host(offerer_address, "udp").map_err(|error| anyhow!("{error}"))?);
	answerer
		.add_local_candidate(Candidate::host(answerer_address, "udp").map_err(|error| anyhow!("{error}"))?);
	let mut changes = offerer.sdp_api();
	changes.add_media(MediaKind::Audio, Direction::RecvOnly, None, None, None);
	changes.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
	let (offer, pending) = changes.apply().context("the offer had no changes")?;
	let offer_text = offer.to_sdp_string();
	let answer: SdpAnswer = answerer
		.sdp_api()
		.accept_offer(offer)
		.map_err(|error| anyhow!("accept the offer: {error}"))?;
	offerer
		.sdp_api()
		.accept_answer(pending, answer)
		.map_err(|error| anyhow!("accept the answer: {error}"))?;
	let observed = Mutex::new(Observed::default());
	let deadline = Instant::now() + wait;
	let mut now = Instant::now();
	while Instant::now() < deadline {
		let from_offerer = drain(&mut offerer, &observed, now)?;
		let from_answerer = drain(&mut answerer, &Mutex::new(Observed::default()), now)?;
		for transmit in &from_offerer {
			record(&observed, &transmit.contents);
			deliver(&mut answerer, transmit, now)?;
		}
		for transmit in &from_answerer {
			deliver(&mut offerer, transmit, now)?;
		}
		if observed.lock().connected {
			break;
		}
		if from_offerer.is_empty() && from_answerer.is_empty() {
			now += Duration::from_millis(10);
			offerer.handle_input(Input::Timeout(now))?;
			answerer.handle_input(Input::Timeout(now))?;
		}
	}
	let observed = observed.into_inner();
	let Some(hello) = observed.hello else {
		bail!("the str0m offerer sent no DTLS ClientHello");
	};
	Ok(Loopback {
		hello,
		hello_bytes: observed.reader.bytes,
		hello_datagrams: observed.reader.datagrams,
		connected: observed.connected,
		offer: offer_text,
	})
}

fn drain(rtc: &mut Rtc, observed: &Mutex<Observed>, now: Instant) -> Result<Vec<Transmit>> {
	let mut sent = Vec::new();
	loop {
		match rtc.poll_output()? {
			Output::Timeout(deadline) => {
				if deadline <= now {
					rtc.handle_input(Input::Timeout(now))?;
					continue;
				}
				return Ok(sent);
			}
			Output::Transmit(transmit) => sent.push(transmit),
			Output::Event(event) => note(observed, &event),
		}
	}
}

fn deliver(rtc: &mut Rtc, transmit: &Transmit, now: Instant) -> Result<()> {
	rtc.handle_input(Input::Receive(
		now,
		Receive {
			proto: Protocol::Udp,
			source: transmit.source,
			destination: transmit.destination,
			contents: (&*transmit.contents).try_into()?,
		},
	))?;
	Ok(())
}
