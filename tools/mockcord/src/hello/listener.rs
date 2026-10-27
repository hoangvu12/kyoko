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
const PAGE: &str = "<!doctype html><meta charset=utf-8><title>kyoko tls-capture</title><p>kyoko tls-capture</p><script>fetch('/fetch').finally(() => { const socket = new WebSocket('wss://' + location.host + '/ws'); socket.onerror = socket.onclose = () => { document.title = 'kyoko tls-capture done'; }; });</script>";
const COOKIES: [&str; 2] = ["kyoko_a=1; Path=/; Secure", "kyoko_b=2; Path=/; Secure"];

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
					h2::response(request.stream, 200, &headers, PAGE.as_bytes())
				}
				Some("/fetch") => h2::response(request.stream, 200, &[("content-type", "text/plain")], b"ok"),
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
				format!(
					"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n{cookies}Content-Length: {}\r\n\r\n{PAGE}",
					PAGE.len()
				)
			}
			Some("/fetch") => {
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
