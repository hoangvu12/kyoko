use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use parking_lot::Mutex;
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::crypto::aws_lc_rs;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::server::NoServerSessionStorage;

use super::client_hello::ClientHello;
use super::h2::{self, Http2Summary, Recorder};
use super::http1::Http1Request;

const HELLO_LIMIT: Duration = Duration::from_secs(10);
const ROUND_TRIP: Duration = Duration::from_millis(50);
const CONNECTION_LIMIT: Duration = Duration::from_secs(60);
const READ_CHUNK: usize = 16 * 1024;
const COOKIES: [&str; 2] = ["kyoko_a=1; Path=/; Secure", "kyoko_b=2; Path=/; Secure"];
const PIXEL: [u8; 43] = [
	0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff,
	0xff, 0xff, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
	0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
];
pub const FETCH_PATH: &str = "/fetch";
pub const IMAGE_PATH: &str = "/image.gif";
pub const WEBSOCKET_PATH: &str = "/ws";
pub const GATEWAY_PATH: &str = "/?v=9&encoding=json";
pub const IMAGE_HOST: &str = "kyoko-cdn.test";
pub const GATEWAY_HOST: &str = "kyoko-gateway.test";
pub const DUMMY_API_HEADERS: [(&str, &str); 6] = [
	("Authorization", "kyoko-tls-capture-dummy-token"),
	("X-Debug-Options", "bugReporterEnabled"),
	("X-Discord-Locale", "en-US"),
	("X-Discord-Timezone", "Etc/UTC"),
	("X-Installation-ID", "0.kyoko-tls-capture-dummy"),
	("X-Super-Properties", "eyJreW9rbyI6ImR1bW15In0="),
];
const JSON_BODY: (&str, &str) = ("Content-Type", "application/json");
pub const API_CALLS: [ApiCall; 4] = [
	ApiCall {
		method: "GET",
		path: "/api/v9/users/@me",
		extra: &[],
	},
	ApiCall {
		method: "POST",
		path: "/api/v9/channels/1/messages",
		extra: &[JSON_BODY],
	},
	ApiCall {
		method: "POST",
		path: "/api/v9/invites/kyoko",
		extra: &[
			JSON_BODY,
			("X-Context-Properties", "eyJsb2NhdGlvbiI6ImR1bW15In0="),
		],
	},
	ApiCall {
		method: "PATCH",
		path: "/api/v9/guilds/1/members/2",
		extra: &[JSON_BODY, ("X-Audit-Log-Reason", "kyoko%20tls-capture")],
	},
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApiCall {
	pub method: &'static str,
	pub path: &'static str,
	pub extra: &'static [(&'static str, &'static str)],
}

impl ApiCall {
	fn script(&self) -> String {
		let headers = DUMMY_API_HEADERS
			.iter()
			.chain(self.extra)
			.map(|(name, value)| format!("'{name}': '{value}'"))
			.collect::<Vec<_>>();
		let body = if self.method == "GET" {
			String::new()
		} else {
			", body: JSON.stringify({ content: 'kyoko tls-capture' })".to_owned()
		};
		format!(
			"await fetch('{}', {{ method: '{}', headers: {{ {} }}{body} }}).catch(() => {{}}); ",
			self.path,
			self.method,
			headers.join(", ")
		)
	}
}

fn page() -> String {
	let calls = API_CALLS.iter().map(ApiCall::script).collect::<String>();
	format!(
		"<!doctype html><meta charset=utf-8><title>kyoko tls-capture</title><p>kyoko tls-capture</p><script>		const port = location.port; 		const socket = (url) => new Promise((done) => {{ 			const socket = new WebSocket(url); socket.onerror = socket.onclose = done; }}); 		const image = (url) => new Promise((done) => {{ 			const element = document.createElement('img'); element.onload = element.onerror = done; 			element.src = url; document.body.append(element); }}); 		(async () => {{ 			await fetch('{FETCH_PATH}').catch(() => {{}}); 			{calls}			await image('https://{IMAGE_HOST}:' + port + '{IMAGE_PATH}'); 			await socket('wss://' + location.host + '{WEBSOCKET_PATH}'); 			await socket('wss://{GATEWAY_HOST}:' + port + '{GATEWAY_PATH}'); 			document.title = 'kyoko tls-capture done'; 		}})();</script>"
	)
}

pub fn is_api_path(path: &str) -> bool {
	API_CALLS.iter().any(|call| call.path == path)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
	HelloOnly,
	Complete,
}

#[derive(Clone, Debug, Default)]
pub struct Connection {
	pub hello: Option<ClientHello>,
	pub alpn: Option<String>,
	pub http2: Option<Http2Summary>,
	pub http1: Option<Http1Request>,
}

type Connections = Arc<Mutex<Vec<Connection>>>;

pub struct HelloListener {
	port: u16,
	root_pem: String,
	connections: Connections,
	task: JoinHandle<()>,
}

impl HelloListener {
	pub async fn start(mode: Mode) -> io::Result<HelloListener> {
		let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
		let port = listener.local_addr()?.port();
		let certificates = certificates().map_err(io::Error::other)?;
		let acceptor = acceptor(&certificates)?;
		let connections = Connections::default();
		let shared = connections.clone();
		let task = tokio::spawn(async move {
			while let Ok((stream, _)) = listener.accept().await {
				let slot = {
					let mut connections = shared.lock();
					connections.push(Connection::default());
					connections.len() - 1
				};
				let record = Record {
					connections: shared.clone(),
					slot,
				};
				let acceptor = acceptor.clone();
				tokio::spawn(async move {
					let _ =
						tokio::time::timeout(CONNECTION_LIMIT, serve(stream, mode, acceptor, record)).await;
				});
			}
		});
		Ok(HelloListener {
			port,
			root_pem: certificates.root_pem,
			connections,
			task,
		})
	}

	pub fn port(&self) -> u16 {
		self.port
	}

	pub fn root_pem(&self) -> &str {
		&self.root_pem
	}

	pub fn connections(&self) -> Vec<Connection> {
		self.connections.lock().clone()
	}
}

impl Drop for HelloListener {
	fn drop(&mut self) {
		self.task.abort();
	}
}

struct Record {
	connections: Connections,
	slot: usize,
}

impl Record {
	fn update(&self, change: impl FnOnce(&mut Connection)) {
		if let Some(connection) = self.connections.lock().get_mut(self.slot) {
			change(connection);
		}
	}
}

async fn serve(mut stream: TcpStream, mode: Mode, acceptor: TlsAcceptor, record: Record) -> io::Result<()> {
	let mut received = Vec::new();
	let hello = tokio::time::timeout(HELLO_LIMIT, async {
		let mut chunk = vec![0; READ_CHUNK];
		loop {
			let read = stream.read(&mut chunk).await?;
			if read == 0 {
				return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
			}
			received.extend_from_slice(&chunk[..read]);
			if let Some((_, hello)) = ClientHello::read(&received).map_err(io::Error::other)? {
				return Ok(hello);
			}
		}
	})
	.await
	.map_err(io::Error::other)??;
	record.update(|connection| connection.hello = Some(hello));
	if mode == Mode::HelloOnly {
		return Ok(());
	}
	let replay = Replay {
		prefix: received,
		at: 0,
		inner: stream,
	};
	let tls = acceptor.accept(replay).await?;
	let alpn = tls
		.get_ref()
		.1
		.alpn_protocol()
		.map(|protocol| String::from_utf8_lossy(protocol).into_owned());
	record.update(|connection| connection.alpn = alpn.clone());
	if alpn.as_deref() == Some("h2") {
		serve_h2(tls, &record).await
	} else {
		serve_http1(tls, &record).await
	}
}

async fn serve_h2<S: AsyncRead + AsyncWrite + Unpin>(mut tls: S, record: &Record) -> io::Result<()> {
	tokio::time::sleep(ROUND_TRIP).await;
	tls.write_all(&h2::server_settings()).await?;
	let mut recorder = Recorder::default();
	let mut chunk = vec![0; READ_CHUNK];
	loop {
		let read = tls.read(&mut chunk).await?;
		if read == 0 {
			return Ok(());
		}
		let requests = recorder.feed(&chunk[..read]);
		let summary = recorder.summary().clone();
		record.update(|connection| connection.http2 = Some(summary));
		let mut reply = Vec::new();
		for _ in 0..recorder.take_settings_acks() {
			reply.extend(h2::settings_ack());
		}
		for request in requests {
			reply.extend(match request.path() {
				Some("/") => {
					let mut headers = vec![("content-type", "text/html; charset=utf-8")];
					headers.extend(COOKIES.iter().map(|cookie| ("set-cookie", *cookie)));
					h2::response(request.stream, 200, &headers, page().as_bytes())
				}
				Some(FETCH_PATH) => {
					h2::response(request.stream, 200, &[("content-type", "text/plain")], b"ok")
				}
				Some(path) if is_api_path(path) => h2::response(
					request.stream,
					200,
					&[("content-type", "application/json")],
					b"{}",
				),
				Some(IMAGE_PATH) => {
					h2::response(request.stream, 200, &[("content-type", "image/gif")], &PIXEL)
				}
				_ => h2::response(request.stream, 404, &[], b""),
			});
		}
		if !reply.is_empty() {
			tls.write_all(&reply).await?;
		}
	}
}

async fn serve_http1<S: AsyncRead + AsyncWrite + Unpin>(mut tls: S, record: &Record) -> io::Result<()> {
	let mut received = Vec::new();
	let mut chunk = vec![0; READ_CHUNK];
	loop {
		let read = tls.read(&mut chunk).await?;
		if read == 0 {
			return Ok(());
		}
		received.extend_from_slice(&chunk[..read]);
		let Some(end) = received.windows(4).position(|window| window == b"\r\n\r\n") else {
			continue;
		};
		let request = Http1Request::parse(&received[..end + 4]);
		received.drain(..end + 4);
		let path = request
			.as_ref()
			.and_then(|request| request.request_line.split(' ').nth(1).map(str::to_owned));
		record.update(|connection| {
			if connection.http1.is_none() {
				connection.http1 = request;
			}
		});
		let reply = match path.as_deref() {
			Some("/") => {
				let cookies = COOKIES
					.iter()
					.map(|cookie| format!("Set-Cookie: {cookie}\r\n"))
					.collect::<String>();
				let page = page();
				format!(
					"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n{cookies}Content-Length: {}\r\n\r\n{page}",
					page.len()
				)
			}
			Some(FETCH_PATH) => {
				"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok".to_owned()
			}
			_ => {
				tls.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
					.await?;
				return tls.shutdown().await;
			}
		};
		tls.write_all(reply.as_bytes()).await?;
	}
}

struct Replay {
	prefix: Vec<u8>,
	at: usize,
	inner: TcpStream,
}

impl AsyncRead for Replay {
	fn poll_read(
		self: Pin<&mut Self>,
		context: &mut Context<'_>,
		buffer: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		let this = self.get_mut();
		if this.at < this.prefix.len() {
			let count = (this.prefix.len() - this.at).min(buffer.remaining());
			buffer.put_slice(&this.prefix[this.at..this.at + count]);
			this.at += count;
			return Poll::Ready(Ok(()));
		}
		Pin::new(&mut this.inner).poll_read(context, buffer)
	}
}

impl AsyncWrite for Replay {
	fn poll_write(self: Pin<&mut Self>, context: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
		Pin::new(&mut self.get_mut().inner).poll_write(context, bytes)
	}

	fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().inner).poll_flush(context)
	}

	fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().inner).poll_shutdown(context)
	}
}

struct Certificates {
	root_pem: String,
	server: CertificateDer<'static>,
	key: PrivateKeyDer<'static>,
}

fn certificates() -> Result<Certificates, rcgen::Error> {
	let root_key = KeyPair::generate()?;
	let mut root_params = CertificateParams::new(Vec::<String>::new())?;
	root_params
		.distinguished_name
		.push(DnType::CommonName, "kyoko tls-capture root");
	root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
	let root = root_params.self_signed(&root_key)?;
	let issuer = Issuer::new(root_params, root_key);
	let server_key = KeyPair::generate()?;
	let mut server_params = CertificateParams::new(vec!["localhost".to_owned()])?;
	server_params
		.distinguished_name
		.push(DnType::CommonName, "localhost");
	let server = server_params.signed_by(&server_key, &issuer)?;
	let key = PrivateKeyDer::try_from(server_key.serialize_der())
		.map_err(|_| rcgen::Error::CouldNotParseKeyPair)?;
	Ok(Certificates {
		root_pem: root.pem(),
		server: server.der().clone(),
		key,
	})
}

fn acceptor(certificates: &Certificates) -> io::Result<TlsAcceptor> {
	let mut config = ServerConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
		.with_safe_default_protocol_versions()
		.map_err(io::Error::other)?
		.with_no_client_auth()
		.with_single_cert(vec![certificates.server.clone()], certificates.key.clone_key())
		.map_err(io::Error::other)?;
	config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
	config.session_storage = Arc::new(NoServerSessionStorage {});
	config.send_tls13_tickets = 0;
	Ok(TlsAcceptor::from(Arc::new(config)))
}
