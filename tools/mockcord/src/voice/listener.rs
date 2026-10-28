use std::io;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::extract::connect_info::Connected;
use axum::serve::{IncomingStream, Listener};
use tokio::net::TcpListener;
#[cfg(not(feature = "tls"))]
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::Voice;
use super::recording::VoiceSocket;

#[cfg(feature = "tls")]
pub(crate) type VoiceStream = tls::Tapped;
#[cfg(not(feature = "tls"))]
pub(crate) type VoiceStream = TcpStream;

const PENDING_CONNECTIONS: usize = 16;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ConnectionId(pub u64);

impl Connected<IncomingStream<'_, VoiceListener>> for ConnectionId {
	fn connect_info(stream: IncomingStream<'_, VoiceListener>) -> Self {
		*stream.remote_addr()
	}
}

pub(crate) struct Bound {
	pub url: String,
	pub root_pem: Option<String>,
	listener: TcpListener,
	#[cfg(feature = "tls")]
	acceptor: tokio_rustls::TlsAcceptor,
}

#[cfg(feature = "tls")]
pub(crate) async fn bind() -> io::Result<Bound> {
	let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
	let certificates = crate::hello::listener::certificates().map_err(io::Error::other)?;
	let acceptor = crate::hello::listener::acceptor(&certificates, &[b"http/1.1"])?;
	Ok(Bound {
		url: format!("wss://localhost:{}", listener.local_addr()?.port()),
		root_pem: Some(certificates.root_pem),
		listener,
		acceptor,
	})
}

#[cfg(not(feature = "tls"))]
pub(crate) async fn bind() -> io::Result<Bound> {
	let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
	Ok(Bound {
		url: format!("ws://{}", listener.local_addr()?),
		root_pem: None,
		listener,
	})
}

pub(crate) struct VoiceListener {
	incoming: mpsc::Receiver<(VoiceStream, ConnectionId)>,
}

impl VoiceListener {
	pub fn start(bound: Bound, voice: Arc<Voice>) -> (Self, JoinHandle<()>) {
		let (sender, incoming) = mpsc::channel(PENDING_CONNECTIONS);
		let accepting = tokio::spawn(async move {
			while let Ok((stream, _)) = bound.listener.accept().await {
				let id = voice.next_connection.fetch_add(1, Ordering::Relaxed);
				voice.record(|log| log.sockets.push(VoiceSocket::new(id)));
				let sender = sender.clone();
				let voice = voice.clone();
				#[cfg(feature = "tls")]
				let acceptor = bound.acceptor.clone();
				tokio::spawn(async move {
					#[cfg(feature = "tls")]
					let stream = tls::accept(stream, acceptor, voice, id).await;
					#[cfg(not(feature = "tls"))]
					let stream = plain(stream, voice);
					if let Ok(stream) = stream {
						let _ = sender.send((stream, ConnectionId(id))).await;
					}
				});
			}
		});
		(Self { incoming }, accepting)
	}
}

#[cfg(not(feature = "tls"))]
fn plain(stream: TcpStream, _voice: Arc<Voice>) -> io::Result<TcpStream> {
	Ok(stream)
}

impl Listener for VoiceListener {
	type Io = VoiceStream;
	type Addr = ConnectionId;

	async fn accept(&mut self) -> (Self::Io, Self::Addr) {
		match self.incoming.recv().await {
			Some(accepted) => accepted,
			None => std::future::pending().await,
		}
	}

	fn local_addr(&self) -> io::Result<Self::Addr> {
		Ok(ConnectionId(0))
	}
}

#[cfg(feature = "tls")]
mod tls {
	use std::io;
	use std::pin::Pin;
	use std::sync::Arc;
	use std::task::{Context, Poll};
	use std::time::Duration;

	use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
	use tokio::net::TcpStream;
	use tokio_rustls::TlsAcceptor;
	use tokio_rustls::server::TlsStream;

	use super::super::Voice;
	use crate::hello::listener::Replay;
	use crate::hello::{ClientHello, Http1Request};

	const HELLO_LIMIT: Duration = Duration::from_secs(10);
	const READ_CHUNK: usize = 16 * 1024;
	const HEAD_END: &[u8; 4] = b"\r\n\r\n";
	const MAX_HEAD_BYTES: usize = 64 * 1024;

	pub(crate) struct Tapped {
		inner: TlsStream<Replay>,
		voice: Arc<Voice>,
		id: u64,
		head: Option<Vec<u8>>,
	}

	pub(crate) async fn accept(
		mut stream: TcpStream,
		acceptor: TlsAcceptor,
		voice: Arc<Voice>,
		id: u64,
	) -> io::Result<Tapped> {
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
		voice.record(|log| {
			if let Some(socket) = log.socket_mut(id) {
				socket.hello = Some(hello);
			}
		});
		let replay = Replay {
			prefix: received,
			at: 0,
			inner: stream,
		};
		let inner = acceptor.accept(replay).await?;
		let alpn = inner
			.get_ref()
			.1
			.alpn_protocol()
			.map(|protocol| String::from_utf8_lossy(protocol).into_owned());
		voice.record(|log| {
			if let Some(socket) = log.socket_mut(id) {
				socket.alpn = alpn;
			}
		});
		Ok(Tapped {
			inner,
			voice,
			id,
			head: Some(Vec::new()),
		})
	}

	impl Tapped {
		fn observe(&mut self, bytes: &[u8]) {
			let Some(head) = self.head.as_mut() else {
				return;
			};
			head.extend_from_slice(bytes);
			let complete = head.windows(HEAD_END.len()).any(|window| window == HEAD_END);
			if !complete && head.len() < MAX_HEAD_BYTES {
				return;
			}
			let request = self.head.take().and_then(|head| Http1Request::parse(&head));
			let id = self.id;
			self.voice.record(|log| {
				if let Some(socket) = log.socket_mut(id) {
					socket.upgrade = request;
				}
			});
		}
	}

	impl AsyncRead for Tapped {
		fn poll_read(
			self: Pin<&mut Self>,
			context: &mut Context<'_>,
			buffer: &mut ReadBuf<'_>,
		) -> Poll<io::Result<()>> {
			let this = self.get_mut();
			let before = buffer.filled().len();
			let polled = Pin::new(&mut this.inner).poll_read(context, buffer);
			if this.head.is_some() && matches!(polled, Poll::Ready(Ok(()))) {
				let filled = buffer.filled()[before..].to_vec();
				this.observe(&filled);
			}
			polled
		}
	}

	impl AsyncWrite for Tapped {
		fn poll_write(
			self: Pin<&mut Self>,
			context: &mut Context<'_>,
			bytes: &[u8],
		) -> Poll<io::Result<usize>> {
			Pin::new(&mut self.get_mut().inner).poll_write(context, bytes)
		}

		fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
			Pin::new(&mut self.get_mut().inner).poll_flush(context)
		}

		fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
			Pin::new(&mut self.get_mut().inner).poll_shutdown(context)
		}
	}
}
