use std::process::ExitCode;

use net::Destination;

enum Trust {
	Nothing,
	Root(String),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
	let mut args = std::env::args().skip(1);
	let Some(port) = args.next().and_then(|port| port.parse::<u16>().ok()) else {
		eprintln!("usage: tls_probe <localhost port> [root.pem]");
		return ExitCode::FAILURE;
	};
	let trust = match args.next() {
		Some(path) => match std::fs::read_to_string(&path) {
			Ok(pem) => Trust::Root(pem),
			Err(error) => {
				eprintln!("tls_probe: read {path}: {error}");
				return ExitCode::FAILURE;
			}
		},
		None => Trust::Nothing,
	};
	match probe(port, trust).await {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("tls_probe: {error}");
			ExitCode::FAILURE
		}
	}
}

async fn probe(port: u16, trust: Trust) -> Result<(), net::Error> {
	let client = match trust {
		Trust::Nothing => {
			net::client_builder(Destination::Document, &net::Trust::Bundled)?.tls_cert_verification(false)
		}
		Trust::Root(pem) => net::client_builder(Destination::Document, &net::Trust::only(pem))?,
	}
	.cookie_store(true)
	.build()?;
	let base = format!("https://localhost:{port}");
	for path in ["/", "/fetch"] {
		match client.get(format!("{base}{path}")).send().await {
			Ok(response) => println!("GET {path}: {} over {:?}", response.status(), response.version()),
			Err(error) => println!("GET {path}: {error}"),
		}
	}
	let socket = client
		.websocket(format!("wss://localhost:{port}/ws"))
		.emulation(net::websocket_emulation());
	match socket.send().await {
		Ok(response) => println!("websocket: {}", response.status()),
		Err(error) => println!("websocket: {error}"),
	}
	Ok(())
}
