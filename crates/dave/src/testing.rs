use std::collections::{HashMap, HashSet, VecDeque};

use openmls::prelude::{
	BasicCredential, ExternalProposal, ExternalSender, GroupEpoch, GroupId, KeyPackageIn, LeafNodeIndex,
	MlsMessageIn, OpenMlsProvider, ProtocolVersion, SenderExtensionIndex, SignaturePublicKey, VLBytes,
};
use serde_json::{Value, json};
use tls_codec::{DeserializeBytes, Serialize};

use crate::error::{DaveError, Result, failure};
use crate::provider::Provider;
use crate::session::{DaveSession, PROTOCOL_VERSION};
use crate::signer::SigningKey;
use crate::transform::{Codec, DaveCrypto};
use crate::wire::{Outgoing, Wire, opcode};

const GATEWAY_IDENTITY: &[u8] = b"kyoko test gateway";
const APPEND_OPERATION: u8 = 0;
const BOGUS_COMMIT: [u8; 4] = [0xde, 0xad, 0xbe, 0xef];

pub struct Member {
	pub user_id: u64,
	pub session: DaveSession,
	crypto: DaveCrypto,
}

impl Member {
	pub fn new(user_id: u64, group: u64) -> Result<Self> {
		let session = DaveSession::new(user_id, group)?;
		let crypto = session.crypto();
		Ok(Self {
			user_id,
			session,
			crypto,
		})
	}

	pub fn start(&mut self) -> Vec<Wire> {
		encode(self.session.on_protocol_version(PROTOCOL_VERSION))
	}

	pub fn receive(&mut self, message: &Wire) -> Vec<Wire> {
		let outgoing = match message {
			Wire::Binary(bytes) => match bytes.split_first() {
				Some((&op, payload)) => self.session.on_binary(op, payload),
				None => Vec::new(),
			},
			Wire::Text(text) => match parse(text) {
				Some((op, data)) => self.on_json(op, &data),
				None => Vec::new(),
			},
		};
		encode(outgoing)
	}

	pub fn encrypt(&self, frame: &[u8]) -> Option<Vec<u8>> {
		self.encrypt_as(Codec::Opus, frame)
	}

	pub fn decrypt(&self, sender: u64, payload: &[u8]) -> Option<Vec<u8>> {
		self.decrypt_as(Codec::Opus, sender, payload)
	}

	pub fn encrypt_as(&self, codec: Codec, frame: &[u8]) -> Option<Vec<u8>> {
		self.crypto.encrypt(codec, frame)
	}

	pub fn decrypt_as(&self, codec: Codec, sender: u64, payload: &[u8]) -> Option<Vec<u8>> {
		self.crypto.decrypt(codec, Some(sender), payload)
	}

	fn on_json(&mut self, op: u8, data: &Value) -> Vec<Outgoing> {
		match op {
			opcode::CLIENTS_CONNECT => {
				for user_id in ids(&data["user_ids"]) {
					self.session.user_joined(user_id);
				}
				Vec::new()
			}
			opcode::CLIENT_DISCONNECT => {
				if let Some(user_id) = id(&data["user_id"]) {
					self.session.user_left(user_id);
				}
				Vec::new()
			}
			_ => self.session.on_json(op, data),
		}
	}
}

fn encode(outgoing: Vec<Outgoing>) -> Vec<Wire> {
	outgoing.iter().map(Outgoing::encode).collect()
}

pub fn parse(text: &str) -> Option<(u8, Value)> {
	let frame: Value = serde_json::from_str(text).ok()?;
	let op = u8::try_from(frame["op"].as_u64()?).ok()?;
	Some((op, frame["d"].clone()))
}

fn id(value: &Value) -> Option<u64> {
	match value {
		Value::String(text) => text.parse().ok(),
		Value::Number(number) => number.as_u64(),
		_ => None,
	}
}

fn ids(value: &Value) -> Vec<u64> {
	value
		.as_array()
		.map(|values| values.iter().filter_map(id).collect())
		.unwrap_or_default()
}

struct Transition {
	id: u16,
	participants: Vec<u64>,
	ready: HashSet<u64>,
}

pub type Mail = Vec<(u64, Wire)>;

pub struct Gateway {
	provider: Provider,
	signer: SigningKey,
	group_id: GroupId,
	epoch: u64,
	transition_id: u16,
	connected: Vec<u64>,
	established: Vec<u64>,
	adds: Vec<(u64, Vec<u8>)>,
	removals: Vec<u64>,
	transition: Option<Transition>,
}

impl Gateway {
	pub fn new(group: u64) -> Result<Self> {
		Ok(Self {
			provider: Provider::default(),
			signer: SigningKey::generate().map_err(failure("signing key"))?,
			group_id: GroupId::from_slice(&group.to_be_bytes()),
			epoch: 0,
			transition_id: 0,
			connected: Vec::new(),
			established: Vec::new(),
			adds: Vec::new(),
			removals: Vec::new(),
			transition: None,
		})
	}

	pub fn epoch(&self) -> u64 {
		self.epoch
	}

	pub fn established(&self) -> &[u64] {
		&self.established
	}

	pub fn join(&mut self, user_id: u64) -> Mail {
		let mut mail = Mail::new();
		let external_sender = self.external_sender();
		mail.push((user_id, binary(opcode::MLS_EXTERNAL_SENDER, &external_sender)));
		if !self.connected.is_empty() {
			mail.push((user_id, clients_connect(&self.connected)));
		}
		if self.established.is_empty() {
			for (_, proposal) in &self.adds {
				mail.push((user_id, proposals(proposal)));
			}
		}
		for existing in &self.connected {
			mail.push((*existing, clients_connect(&[user_id])));
		}
		self.connected.push(user_id);
		mail
	}

	pub fn leave(&mut self, user_id: u64, leaf_index: Option<u32>) -> Result<Mail> {
		self.connected.retain(|user| *user != user_id);
		self.adds.retain(|(user, _)| *user != user_id);
		let mut mail: Mail = self
			.connected
			.iter()
			.map(|user| (*user, client_disconnect(user_id)))
			.collect();
		mail.extend(self.propose_removal(user_id, leaf_index)?);
		Ok(mail)
	}

	pub fn invalid(&mut self, user_id: u64, leaf_index: Option<u32>) -> Result<Mail> {
		self.adds.retain(|(user, _)| *user != user_id);
		self.propose_removal(user_id, leaf_index)
	}

	pub fn downgrade(&mut self) -> Mail {
		self.transition_id = self.transition_id.wrapping_add(1);
		let id = self.transition_id;
		let participants = self.established.clone();
		self.transition = Some(Transition {
			id,
			participants: participants.clone(),
			ready: HashSet::new(),
		});
		participants
			.into_iter()
			.map(|user| (user, prepare_transition(id, 0)))
			.collect()
	}

	pub fn upgrade(&mut self) -> Mail {
		self.epoch = 0;
		self.established.clear();
		self.adds.clear();
		self.removals.clear();
		self.transition = None;
		self.connected
			.iter()
			.map(|user| (*user, prepare_epoch(PROTOCOL_VERSION, 1)))
			.collect()
	}

	pub fn bogus_commit(&mut self, user_id: u64) -> Mail {
		self.transition_id = self.transition_id.wrapping_add(1);
		let id = self.transition_id;
		vec![(
			user_id,
			transitioned(opcode::MLS_ANNOUNCE_COMMIT, id, &BOGUS_COMMIT),
		)]
	}

	pub fn receive(&mut self, from: u64, message: &Wire) -> Result<Mail> {
		match message {
			Wire::Binary(bytes) => match bytes.split_first() {
				Some((&opcode::MLS_KEY_PACKAGE, payload)) => self.on_key_package(from, payload),
				Some((&opcode::MLS_COMMIT_WELCOME, payload)) => self.on_commit_welcome(from, payload),
				_ => Ok(Mail::new()),
			},
			Wire::Text(text) => match parse(text) {
				Some((opcode::TRANSITION_READY, data)) => {
					Ok(self.on_ready(from, data["transition_id"].as_u64()))
				}
				_ => Ok(Mail::new()),
			},
		}
	}

	fn propose_removal(&mut self, user_id: u64, leaf_index: Option<u32>) -> Result<Mail> {
		if !self.established.contains(&user_id) {
			return Ok(Mail::new());
		}
		let leaf_index = leaf_index.ok_or_else(|| DaveError::new("no leaf index for the removed user"))?;
		self.removals.push(user_id);
		let proposal = ExternalProposal::new_remove::<Provider>(
			LeafNodeIndex::new(leaf_index),
			self.group_id.clone(),
			GroupEpoch::from(self.epoch),
			&self.signer,
			SenderExtensionIndex::new(0),
		)
		.map_err(failure("remove proposal"))?
		.tls_serialize_detached()
		.map_err(failure("remove proposal"))?;
		Ok(self
			.established
			.iter()
			.filter(|user| **user != user_id)
			.map(|user| (*user, proposals(&proposal)))
			.collect())
	}

	fn external_sender(&self) -> Vec<u8> {
		ExternalSender::new(
			SignaturePublicKey::from(self.signer.public()),
			BasicCredential::new(GATEWAY_IDENTITY.to_vec()).into(),
		)
		.tls_serialize_detached()
		.expect("an external sender serializes")
	}

	fn on_key_package(&mut self, from: u64, payload: &[u8]) -> Result<Mail> {
		let key_package = KeyPackageIn::tls_deserialize_exact_bytes(payload)
			.map_err(failure("key package"))?
			.validate(self.provider.crypto(), ProtocolVersion::Mls10)
			.map_err(failure("key package"))?;
		let proposal = ExternalProposal::new_add::<Provider>(
			key_package,
			self.group_id.clone(),
			GroupEpoch::from(self.epoch),
			&self.signer,
			SenderExtensionIndex::new(0),
		)
		.map_err(failure("add proposal"))?
		.tls_serialize_detached()
		.map_err(failure("add proposal"))?;
		self.adds.retain(|(user, _)| *user != from);
		self.adds.push((from, proposal.clone()));
		let recipients = if self.established.is_empty() {
			&self.connected
		} else {
			&self.established
		};
		Ok(recipients
			.iter()
			.filter(|user| **user != from)
			.map(|user| (*user, proposals(&proposal)))
			.collect())
	}

	fn on_commit_welcome(&mut self, from: u64, payload: &[u8]) -> Result<Mail> {
		let (commit, welcome) = MlsMessageIn::tls_deserialize_bytes(payload).map_err(failure("commit"))?;
		let epoch = commit
			.try_into_protocol_message()
			.map_err(failure("commit"))?
			.epoch()
			.as_u64();
		if epoch != self.epoch {
			return Ok(Mail::new());
		}
		let commit_bytes = &payload[..payload.len() - welcome.len()];
		self.epoch += 1;
		self.transition_id = self.transition_id.wrapping_add(1);
		let id = self.transition_id;
		let mut recipients = self.established.clone();
		if !recipients.contains(&from) {
			recipients.push(from);
		}
		let welcomed: Vec<u64> = self
			.adds
			.iter()
			.map(|(user, _)| *user)
			.filter(|user| *user != from && self.connected.contains(user))
			.collect();
		let mut participants: Vec<u64> = recipients
			.iter()
			.copied()
			.filter(|user| !self.removals.contains(user))
			.collect();
		participants.extend(welcomed.iter().copied());
		self.established = participants.clone();
		self.adds.clear();
		self.removals.clear();
		self.transition = Some(Transition {
			id,
			participants,
			ready: HashSet::new(),
		});
		let mut mail = Mail::new();
		for user in recipients {
			if self.connected.contains(&user) {
				mail.push((user, transitioned(opcode::MLS_ANNOUNCE_COMMIT, id, commit_bytes)));
			}
		}
		for user in welcomed {
			mail.push((user, transitioned(opcode::MLS_WELCOME, id, welcome)));
		}
		Ok(mail)
	}

	fn on_ready(&mut self, from: u64, id: Option<u64>) -> Mail {
		let Some(transition) = self.transition.as_mut() else {
			return Mail::new();
		};
		if id != Some(u64::from(transition.id)) {
			return Mail::new();
		}
		transition.ready.insert(from);
		let done = transition
			.participants
			.iter()
			.all(|user| transition.ready.contains(user));
		if !done {
			return Mail::new();
		}
		let Some(transition) = self.transition.take() else {
			return Mail::new();
		};
		transition
			.participants
			.into_iter()
			.map(|user| (user, execute(transition.id)))
			.collect()
	}
}

fn binary(op: u8, payload: &[u8]) -> Wire {
	let mut bytes = Vec::with_capacity(1 + payload.len());
	bytes.push(op);
	bytes.extend_from_slice(payload);
	Wire::Binary(bytes)
}

fn transitioned(op: u8, id: u16, payload: &[u8]) -> Wire {
	let mut body = id.to_be_bytes().to_vec();
	body.extend_from_slice(payload);
	binary(op, &body)
}

fn proposals(proposal: &[u8]) -> Wire {
	let list = VLBytes::new(proposal.to_vec())
		.tls_serialize_detached()
		.expect("a proposal list serializes");
	let mut payload = vec![APPEND_OPERATION];
	payload.extend_from_slice(&list);
	binary(opcode::MLS_PROPOSALS, &payload)
}

fn text(op: u8, data: Value) -> Wire {
	Wire::Text(json!({ "op": op, "d": data }).to_string())
}

fn clients_connect(user_ids: &[u64]) -> Wire {
	let ids: Vec<String> = user_ids.iter().map(u64::to_string).collect();
	text(opcode::CLIENTS_CONNECT, json!({ "user_ids": ids }))
}

fn client_disconnect(user_id: u64) -> Wire {
	text(
		opcode::CLIENT_DISCONNECT,
		json!({ "user_id": user_id.to_string() }),
	)
}

fn execute(id: u16) -> Wire {
	text(opcode::EXECUTE_TRANSITION, json!({ "transition_id": id }))
}

fn prepare_transition(id: u16, version: u8) -> Wire {
	text(
		opcode::PREPARE_TRANSITION,
		json!({ "transition_id": id, "protocol_version": version }),
	)
}

fn prepare_epoch(version: u8, epoch: u64) -> Wire {
	text(
		opcode::PREPARE_EPOCH,
		json!({ "protocol_version": version, "epoch": epoch }),
	)
}

enum Event {
	ToGateway(u64, Wire),
	ToUser(u64, Wire),
}

pub struct Harness {
	pub gateway: Gateway,
	group: u64,
	members: HashMap<u64, Member>,
	outboxes: HashMap<u64, VecDeque<Wire>>,
}

impl Harness {
	pub fn new(group: u64) -> Result<Self> {
		Ok(Self {
			gateway: Gateway::new(group)?,
			group,
			members: HashMap::new(),
			outboxes: HashMap::new(),
		})
	}

	pub fn join(&mut self, user_id: u64) -> Result<()> {
		let mut member = Member::new(user_id, self.group)?;
		let mut events = to_users(self.gateway.join(user_id));
		for message in member.start() {
			events.push_back(Event::ToGateway(user_id, message));
		}
		self.members.insert(user_id, member);
		self.run(events)
	}

	pub fn join_external(&mut self, user_id: u64) -> Result<()> {
		self.outboxes.insert(user_id, VecDeque::new());
		let events = to_users(self.gateway.join(user_id));
		self.run(events)
	}

	pub fn deliver(&mut self, from: u64, message: Wire) -> Result<()> {
		self.run(VecDeque::from([Event::ToGateway(from, message)]))
	}

	pub fn take_outbox(&mut self, user_id: u64) -> Vec<Wire> {
		self.outboxes
			.get_mut(&user_id)
			.map(|outbox| outbox.drain(..).collect())
			.unwrap_or_default()
	}

	pub fn leave(&mut self, user_id: u64) -> Result<()> {
		let leaf_index = self.leaf_index(user_id);
		self.members.remove(&user_id);
		self.outboxes.remove(&user_id);
		let events = to_users(self.gateway.leave(user_id, leaf_index)?);
		self.run(events)
	}

	pub fn downgrade(&mut self) -> Result<()> {
		let events = to_users(self.gateway.downgrade());
		self.run(events)
	}

	pub fn upgrade(&mut self) -> Result<()> {
		let events = to_users(self.gateway.upgrade());
		self.run(events)
	}

	pub fn bogus_commit(&mut self, user_id: u64) -> Result<()> {
		let events = to_users(self.gateway.bogus_commit(user_id));
		self.run(events)
	}

	pub fn member(&self, user_id: u64) -> Option<&Member> {
		self.members.get(&user_id)
	}

	pub fn member_mut(&mut self, user_id: u64) -> Option<&mut Member> {
		self.members.get_mut(&user_id)
	}

	pub fn members(&self) -> impl Iterator<Item = &Member> {
		self.members.values()
	}

	fn leaf_index(&self, user_id: u64) -> Option<u32> {
		self.members
			.values()
			.filter(|member| member.user_id != user_id)
			.find_map(|member| member.session.leaf_index(user_id))
	}

	fn run(&mut self, mut events: VecDeque<Event>) -> Result<()> {
		while let Some(event) = events.pop_front() {
			match event {
				Event::ToGateway(from, message) => {
					let mail = if is_invalid_commit(&message) {
						let leaf_index = self.leaf_index(from);
						self.gateway.invalid(from, leaf_index)?
					} else {
						self.gateway.receive(from, &message)?
					};
					events.extend(to_users(mail));
				}
				Event::ToUser(to, message) => {
					if let Some(outbox) = self.outboxes.get_mut(&to) {
						outbox.push_back(message);
					} else if let Some(member) = self.members.get_mut(&to) {
						for reply in member.receive(&message) {
							events.push_back(Event::ToGateway(to, reply));
						}
					}
				}
			}
		}
		Ok(())
	}
}

fn to_users(mail: Mail) -> VecDeque<Event> {
	mail.into_iter()
		.map(|(to, message)| Event::ToUser(to, message))
		.collect()
}

fn is_invalid_commit(message: &Wire) -> bool {
	match message {
		Wire::Text(text) => parse(text).is_some_and(|(op, _)| op == opcode::MLS_INVALID_COMMIT_WELCOME),
		Wire::Binary(_) => false,
	}
}
