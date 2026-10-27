use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::Arc;

use bridge::{Drop, Rail, RailFolder, RailGuild, RailItem};
use media::{Gallery, ImageName};
use protocol::Snowflake;
use state::{Changes, State};
use store::Store;
use zgx_script::{ScriptHost, Shared, Value};

use crate::stable::StableRows;
use crate::ui::{Link, record};

const ICON_POINTS: f64 = 48.;
const OPEN_FOLDERS_KEY: &str = "rail.open_folders";
const FOLDER_TINT_ALPHA: u32 = 0x66;
const SERVER_DIRECTORY: &str = "https://discord.com/servers";

pub(crate) struct RailView {
	rail: RefCell<Rail>,
	icons: RefCell<HashMap<(Snowflake, String), ImageName>>,
	open_folders: RefCell<BTreeSet<u64>>,
	store: RefCell<Option<Arc<Store>>>,
	items: Shared,
	home_mentions: Shared,
	open: Shared,
	stable: StableRows,
}

impl RailView {
	pub(crate) fn new(host: &ScriptHost) -> Self {
		Self {
			rail: RefCell::new(Rail::default()),
			icons: RefCell::new(HashMap::new()),
			open_folders: RefCell::new(BTreeSet::new()),
			store: RefCell::new(None),
			items: host.shared("rail.items", Value::List(Rc::new([]))),
			home_mentions: host.shared("rail.home_mentions", 0.),
			open: host.shared("rail.open_folders", Value::List(Rc::new([]))),
			stable: StableRows::default(),
		}
	}

	pub(crate) fn restore(&self, store: Arc<Store>) {
		*self.store.borrow_mut() = Some(store);
		self.reload_open();
	}

	pub(crate) fn reset(&self) {
		*self.rail.borrow_mut() = Rail::default();
		self.icons.borrow_mut().clear();
		self.stable.forget();
		self.items.set(Value::List(Rc::new([])));
		self.home_mentions.set(0.0.into());
		self.reload_open();
	}

	fn reload_open(&self) {
		let Some(store) = self.store.borrow().clone() else {
			return;
		};
		let saved = store
			.db
			.kv_get(OPEN_FOLDERS_KEY)
			.ok()
			.flatten()
			.map(|bytes| parse_ids(&String::from_utf8_lossy(&bytes)))
			.unwrap_or_default();
		*self.open_folders.borrow_mut() = saved.into_iter().collect();
		self.publish_open();
	}

	pub(crate) fn apply(&self, state: &State, changes: &Changes, images: Icons<'_>) -> Option<Vec<String>> {
		if !self.rail.borrow_mut().apply(state, changes) {
			return None;
		}
		let rail = self.rail.borrow();
		let names: Vec<(Snowflake, Option<String>)> = rail
			.guilds()
			.map(|guild| (guild.id, self.icon_name(guild, &images)))
			.collect();
		let icons: HashMap<Snowflake, String> = names
			.iter()
			.filter_map(|(id, name)| name.clone().map(|name| (*id, name)))
			.collect();
		let items = rail.items().iter().map(|item| item_value(item, &icons));
		self.items.set(Value::List(self.stable.rows(items)));
		self.home_mentions.set(f64::from(rail.home_mentions()).into());
		Some(icons.into_values().collect())
	}

	fn icon_name(&self, guild: &RailGuild, images: &Icons<'_>) -> Option<String> {
		let hash = guild.icon.as_ref()?;
		let key = (guild.id, hash.clone());
		let mut icons = self.icons.borrow_mut();
		let name = icons.entry(key).or_insert_with(|| {
			let request =
				bridge::guild_icon_request(images.cdn, guild.id, hash, ICON_POINTS, images.scale, false);
			images.gallery.request(request)
		});
		Some(name.as_str().to_owned())
	}

	pub(crate) fn reveal(&self, guild_id: Snowflake) {
		let folder = self.rail.borrow().folder_of(guild_id);
		if let Some(folder) = folder
			&& self.open_folders.borrow_mut().insert(folder)
		{
			self.save_open();
		}
	}

	pub(crate) fn open_all(&self) {
		let folders: Vec<u64> = self
			.rail
			.borrow()
			.items()
			.iter()
			.filter_map(|item| match item {
				RailItem::Folder(folder) => Some(folder.id),
				RailItem::Guild(_) => None,
			})
			.collect();
		self.open_folders.borrow_mut().extend(folders);
		self.save_open();
	}

	fn toggle(&self, folder: u64) {
		{
			let mut open = self.open_folders.borrow_mut();
			if !open.remove(&folder) {
				open.insert(folder);
			}
		}
		self.save_open();
	}

	fn save_open(&self) {
		self.publish_open();
		let text = self
			.open_folders
			.borrow()
			.iter()
			.map(u64::to_string)
			.collect::<Vec<_>>()
			.join(",");
		if let Some(store) = self.store.borrow().as_ref()
			&& let Err(error) = store.db.kv_set(OPEN_FOLDERS_KEY, text.as_bytes())
		{
			tracing::warn!(%error, "the open folders were not saved");
		}
	}

	fn publish_open(&self) {
		let ids = self
			.open_folders
			.borrow()
			.iter()
			.map(|id| Value::from(id.to_string().as_str()))
			.collect();
		self.open.set(Value::List(ids));
	}
}

pub(crate) struct Icons<'a> {
	pub gallery: &'a Gallery,
	pub cdn: &'a str,
	pub scale: f64,
}

pub(crate) fn install_actions(host: &ScriptHost, link: &Rc<Link>) {
	let actions = Rc::downgrade(link);
	host.action("rail.toggle_folder", move |argument| {
		if let (Some(link), Some(id)) = (actions.upgrade(), id_of(&argument)) {
			link.rail().toggle(id);
		}
	});
	let actions = Rc::downgrade(link);
	host.action("rail.drop", move |argument| {
		if let Some(link) = actions.upgrade() {
			drop_on(&link, &argument);
		}
	});
	let actions = Rc::downgrade(link);
	host.action("rail.select_home", move |_| {
		if let Some(link) = actions.upgrade() {
			link.select_home();
		}
	});
	host.action("rail.discover", |_| {
		if let Err(error) = platform::open::open_url(SERVER_DIRECTORY) {
			tracing::warn!(%error, "could not open the server directory");
		}
	});
}

fn drop_on(link: &Link, argument: &Value) {
	let Some(drop) = drop_of(argument) else {
		return;
	};
	let Some(shared) = link.state() else {
		return;
	};
	let command = {
		let state = shared.read();
		match (field_id(argument, "guild"), field_id(argument, "folder")) {
			(Some(guild), _) => {
				bridge::reorder_command(&state, Snowflake(guild), drop, bridge::new_folder_id())
			}
			(None, Some(folder)) => bridge::reorder_folder_command(&state, folder, drop),
			(None, None) => None,
		}
	};
	if let Some(command) = command {
		link.send(command);
	}
}

fn drop_of(argument: &Value) -> Option<Drop> {
	let target = field_id(argument, "target");
	match (field_text(argument, "drop")?, target) {
		("end", _) => Some(Drop::End),
		("before_guild", Some(id)) => Some(Drop::BeforeGuild(Snowflake(id))),
		("onto_guild", Some(id)) => Some(Drop::OntoGuild(Snowflake(id))),
		("before_folder", Some(id)) => Some(Drop::BeforeFolder(id)),
		("onto_folder", Some(id)) => Some(Drop::OntoFolder(id)),
		_ => None,
	}
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
	match value {
		Value::Map(fields) => fields
			.iter()
			.find(|(name, _)| name.as_ref() == key)
			.map(|(_, value)| value),
		_ => None,
	}
}

fn field_text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
	match field(value, key)? {
		Value::Str(text) => Some(text),
		_ => None,
	}
}

fn field_id(value: &Value, key: &str) -> Option<u64> {
	field(value, key).and_then(id_of)
}

fn id_of(value: &Value) -> Option<u64> {
	match value {
		Value::Str(text) => text.parse().ok(),
		_ => None,
	}
}

fn parse_ids(text: &str) -> Vec<u64> {
	text.split(',').filter_map(|id| id.trim().parse().ok()).collect()
}

fn hex(color: u32) -> String {
	format!("#{:06x}", color & 0xff_ffff)
}

fn tint(color: u32) -> String {
	format!("#{:06x}{FOLDER_TINT_ALPHA:02x}", color & 0xff_ffff)
}

fn guild_value(guild: &RailGuild, icons: &HashMap<Snowflake, String>) -> Value {
	let icon = icons
		.get(&guild.id)
		.map(|name| format!("host:{name}"))
		.unwrap_or_default();
	record([
		("kind", "guild".into()),
		("key", format!("g{}", guild.id).as_str().into()),
		("id", guild.id.to_string().as_str().into()),
		("name", guild.name.as_str().into()),
		("acronym", guild.acronym.as_str().into()),
		(
			"letter",
			guild.acronym.chars().take(1).collect::<String>().as_str().into(),
		),
		("icon", icon.as_str().into()),
		("unread", guild.unread.into()),
		("mentions", f64::from(guild.mentions).into()),
	])
}

fn folder_value(folder: &RailFolder, icons: &HashMap<Snowflake, String>) -> Value {
	let guilds = folder
		.guilds
		.iter()
		.map(|guild| guild_value(guild, icons))
		.collect();
	record([
		("kind", "folder".into()),
		("key", format!("f{}", folder.id).as_str().into()),
		("id", folder.id.to_string().as_str().into()),
		("name", folder.name.as_str().into()),
		("color", hex(folder.color).as_str().into()),
		("tint", tint(folder.color).as_str().into()),
		("unread", folder.unread().into()),
		("mentions", f64::from(folder.mentions()).into()),
		("guilds", Value::List(guilds)),
	])
}

fn item_value(item: &RailItem, icons: &HashMap<Snowflake, String>) -> Value {
	match item {
		RailItem::Guild(guild) => guild_value(guild, icons),
		RailItem::Folder(folder) => folder_value(folder, icons),
	}
}
