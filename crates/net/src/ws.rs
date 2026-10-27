use std::time::Duration;

use wreq::header::HeaderMap;
use wreq::{Client, StatusCode};

pub use wreq::ws::WebSocket;
pub use wreq::ws::message::{CloseCode, CloseFrame, Message, Utf8Bytes};

use crate::chrome::websocket_emulation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
	pub max_frame: usize,
	pub max_message: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
	#[error("websocket connect timed out")]
	TimedOut,
	#[error("websocket upgrade refused with http status {0}")]
	Refused(StatusCode),
	#[error("websocket connect failed: {0}")]
	Failed(wreq::Error),
}

pub async fn connect(
	client: &Client,
	url: &str,
	headers: HeaderMap,
	limits: Limits,
	timeout: Duration,
) -> Result<WebSocket, ConnectError> {
	let request = client
		.websocket(url)
		.emulation(websocket_emulation())
		.headers(headers)
		.max_frame_size(limits.max_frame)
		.max_message_size(limits.max_message);
	let upgrade = async {
		let response = request.send().await.map_err(failed)?;
		let status = response.status();
		if status != StatusCode::SWITCHING_PROTOCOLS {
			return Err(ConnectError::Refused(status));
		}
		response.into_websocket().await.map_err(failed)
	};
	tokio::time::timeout(timeout, upgrade)
		.await
		.map_err(|_| ConnectError::TimedOut)?
}

fn failed(error: wreq::Error) -> ConnectError {
	ConnectError::Failed(error.without_uri())
}

pub fn is_oversized(error: &wreq::Error) -> bool {
	let mut cause = std::error::Error::source(error);
	while let Some(current) = cause {
		if let Some(tungstenite::Error::Capacity(_)) = current.downcast_ref::<tungstenite::Error>() {
			return true;
		}
		cause = current.source();
	}
	false
}

pub async fn close(socket: &mut WebSocket, code: u16, reason: &'static str) -> Result<(), wreq::Error> {
	let frame = CloseFrame {
		code: CloseCode::from(code),
		reason: Utf8Bytes::from(reason),
	};
	socket.send(Message::Close(Some(frame))).await
}
