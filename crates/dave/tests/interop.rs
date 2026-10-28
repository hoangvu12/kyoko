use std::num::NonZeroU16;

use dave::testing::{Harness, parse};
use dave::{MARKER, Wire, opcode};
use davey::{DaveSession as Davey, MediaType, ProposalsOperationType};
use serde_json::json;

const CHANNEL: u64 = 1_027_446_311_209_582_603;
const KYOKO: u64 = 104_694_319_306_248_192;
const DAVEY: u64 = 163_427_291_049_984_000;
const FRAMES: [&[u8]; 3] = [
	&[0x78, 0x0b, 0xe4, 0xc1, 0x36, 0xec, 0xc5, 0x80],
	&[0x78, 0x2a, 0x01, 0x93, 0x55, 0x00, 0xfe, 0x10, 0x42, 0x77],
	&[0xfc, 0xff, 0xfe, 0x01],
];
const TRANSITION_ID_BYTES: usize = 2;

struct External {
	session: Davey,
}

impl External {
	fn new() -> Self {
		let version = NonZeroU16::new(u16::from(dave::PROTOCOL_VERSION)).expect("DAVE v1");
		Self {
			session: Davey::new(version, DAVEY, CHANNEL, None).expect("davey starts"),
		}
	}

	fn key_package(&mut self) -> Wire {
		let mut bytes = vec![opcode::MLS_KEY_PACKAGE];
		bytes.extend(
			self.session
				.create_key_package()
				.expect("davey makes a key package"),
		);
		Wire::Binary(bytes)
	}

	fn receive(&mut self, message: &Wire) -> Vec<Wire> {
		match message {
			Wire::Binary(bytes) => {
				let (&op, payload) = bytes.split_first().expect("a binary DAVE frame has an opcode");
				self.on_binary(op, payload)
			}
			Wire::Text(text) => {
				let (op, _) = parse(text).expect("the gateway sends JSON frames");
				assert_ne!(op, opcode::MLS_INVALID_COMMIT_WELCOME);
				Vec::new()
			}
		}
	}

	fn on_binary(&mut self, op: u8, payload: &[u8]) -> Vec<Wire> {
		match op {
			opcode::MLS_EXTERNAL_SENDER => {
				self.session
					.set_external_sender(payload)
					.expect("davey takes the external sender");
				Vec::new()
			}
			opcode::MLS_PROPOSALS => {
				let (&operation, list) = payload.split_first().expect("proposals carry an operation");
				let operation = if operation == 0 {
					ProposalsOperationType::APPEND
				} else {
					ProposalsOperationType::REVOKE
				};
				let commit = self
					.session
					.process_proposals(operation, list, Some(&[KYOKO, DAVEY]))
					.expect("davey accepts the proposals");
				commit
					.map(|commit| {
						let mut bytes = vec![opcode::MLS_COMMIT_WELCOME];
						bytes.extend(commit.commit);
						bytes.extend(commit.welcome.unwrap_or_default());
						Wire::Binary(bytes)
					})
					.into_iter()
					.collect()
			}
			opcode::MLS_ANNOUNCE_COMMIT => {
				let (id, commit) = payload.split_at(TRANSITION_ID_BYTES);
				self.session
					.process_commit(commit)
					.expect("davey merges the commit");
				vec![ready(id)]
			}
			opcode::MLS_WELCOME => {
				let (id, welcome) = payload.split_at(TRANSITION_ID_BYTES);
				self.session
					.process_welcome(welcome)
					.expect("davey joins from the welcome");
				vec![ready(id)]
			}
			_ => Vec::new(),
		}
	}
}

fn ready(id: &[u8]) -> Wire {
	let id = u16::from_be_bytes(id.try_into().expect("a transition id is two bytes"));
	Wire::Text(json!({ "op": opcode::TRANSITION_READY, "d": { "transition_id": id } }).to_string())
}

fn settle(harness: &mut Harness, external: &mut External) {
	loop {
		let mail = harness.take_outbox(DAVEY);
		if mail.is_empty() {
			return;
		}
		for message in mail {
			for reply in external.receive(&message) {
				harness
					.deliver(DAVEY, reply)
					.expect("the gateway takes davey's reply");
			}
		}
	}
}

fn exchange(harness: &Harness, external: &mut External) {
	let kyoko = harness.member(KYOKO).expect("kyoko is a member");
	assert!(external.session.is_ready(), "davey has its media keys");
	for frame in FRAMES {
		let sealed = kyoko
			.encrypt(frame)
			.expect("kyoko encrypts once the group is set up");
		assert!(sealed.ends_with(&MARKER), "kyoko's frame carries the DAVE marker");
		assert_ne!(&sealed[..frame.len()], frame, "kyoko's frame is encrypted");
		let opened = external
			.session
			.decrypt(KYOKO, MediaType::AUDIO, &sealed)
			.expect("davey decrypts kyoko's frame");
		assert_eq!(opened, frame);
		let sealed = external
			.session
			.encrypt_opus(frame)
			.expect("davey encrypts")
			.into_owned();
		assert!(sealed.ends_with(&MARKER), "davey's frame carries the DAVE marker");
		let opened = kyoko
			.decrypt(DAVEY, &sealed)
			.expect("kyoko decrypts davey's frame");
		assert_eq!(opened, frame);
	}
}

fn in_one_group(harness: &Harness) {
	assert_eq!(harness.gateway.epoch(), 1);
	let kyoko = harness.member(KYOKO).expect("kyoko is a member");
	assert_eq!(kyoko.session.epoch(), Some(1));
	let members = kyoko.session.members();
	assert!(members.contains(&DAVEY) && members.contains(&KYOKO));
}

#[test]
fn davey_commits_kyoko_into_the_group_and_both_decrypt_each_other() {
	let mut harness = Harness::new(CHANNEL).expect("the gateway starts");
	harness.join(KYOKO).expect("kyoko joins");
	let mut external = External::new();
	harness.join_external(DAVEY).expect("davey connects");
	settle(&mut harness, &mut external);
	in_one_group(&harness);
	exchange(&harness, &mut external);
}

#[test]
fn kyoko_commits_davey_into_the_group_and_both_decrypt_each_other() {
	let mut harness = Harness::new(CHANNEL).expect("the gateway starts");
	let mut external = External::new();
	harness.join_external(DAVEY).expect("davey connects");
	settle(&mut harness, &mut external);
	let key_package = external.key_package();
	harness
		.deliver(DAVEY, key_package)
		.expect("the gateway takes davey's key package");
	settle(&mut harness, &mut external);
	harness.join(KYOKO).expect("kyoko joins");
	settle(&mut harness, &mut external);
	in_one_group(&harness);
	exchange(&harness, &mut external);
}
