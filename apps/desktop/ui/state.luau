export type RailGuild = {
	kind: string,
	key: string,
	id: string,
	name: string,
	acronym: string,
	letter: string,
	icon: string,
	unread: boolean,
	mentions: number,
}
export type RailFolder = {
	kind: string,
	key: string,
	id: string,
	name: string,
	color: string,
	tint: string,
	unread: boolean,
	mentions: number,
	guilds: { RailGuild },
}
export type RailItem = RailGuild | RailFolder
export type VoiceMember = {
	key: string,
	id: string,
	name: string,
	avatar: string,
	muted: boolean,
	deafened: boolean,
	video: string,
	live: boolean,
}
export type SidebarRow = {
	kind: string,
	key: string,
	id: string,
	name: string,
	collapsed: boolean?,
	type: string?,
	icon: string?,
	unread: boolean?,
	mentions: number?,
	muted: boolean,
	locked: boolean?,
	joined: boolean?,
	selectable: boolean?,
	thread: boolean?,
	last_thread: boolean?,
	voice: { VoiceMember }?,
}
export type GuildHeader = {
	unread: boolean,
	muted: boolean,
	owner: boolean,
	can_invite: boolean,
	can_manage_channels: boolean,
	can_open_settings: boolean,
}
export type DmRow = {
	key: string,
	id: string,
	name: string,
	avatar: string,
	status: string,
	group: boolean,
	members: number,
	unread: boolean,
	mentions: number,
}
export type ComposerReplace = { serial: number, text: string, caret: number }
export type ComposerReply = { active: boolean, name: string?, ping: boolean?, own: boolean? }
export type ComposerFile = { id: string, name: string, size: string, image: string }
export type ComposerSuggestion = { key: string, label: string, detail: string, kind: string, image: string, glyph: string, icon: string }
export type ComposerNotice = { kind: string, text: string?, confirmable: boolean?, nonce: string? }
export type MemberWindow = {
	first: number,
	rows: { any },
}
export type MemberJump = {
	serial: number,
	offset: number,
}
export type SettingsAccount = { name: string, username: string, email: string, phone: string, accent: string, status: string, avatar: string }
export type SettingsAbout = { data_dir: string, version: string }
export type UserPanel = {
	name: string,
	subtitle: string,
	emoji: string,
	emoji_image: string,
	status: string,
	latency: string,
}
export type SearchHit = { key: string, id: string, author: string, avatar: string, time: string, place: string, text: string }
export type SearchPanel = {
	open: boolean,
	title: string,
	notice: string,
	busy: boolean,
	more: boolean,
}

local M = {}

M.session = {
	status = ui.shared("session.status", "Connecting"),
	storage_warning = ui.shared("session.storage_warning", ""),
}

M.login = {
	screen = ui.shared("login.screen", "login"),
	acknowledged = ui.shared("login.acknowledged", false),
	checking = ui.shared("login.checking", false),
	message = ui.shared("login.message", ""),
	error = ui.shared("login.error", false),
	unsaved = ui.shared("login.unsaved", false),
}

M.rail = {
	items = ui.shared("rail.items", {} :: { RailItem }),
	home_mentions = ui.shared("rail.home_mentions", 0),
	open_folders = ui.shared("rail.open_folders", {} :: { string }),
}

M.sidebar = {
	rows = ui.shared("sidebar.rows", {} :: { SidebarRow }),
	guild_id = ui.shared("sidebar.guild_id", ""),
	guild_name = ui.shared("sidebar.guild_name", ""),
	header = ui.shared("sidebar.header", {
		unread = false,
		muted = false,
		owner = false,
		can_invite = false,
		can_manage_channels = false,
		can_open_settings = false,
	} :: GuildHeader),
}

M.dms = {
	rows = ui.shared("dms.rows", {} :: { DmRow }),
	requests = ui.shared("dms.requests", 0),
}

M.home = {
	page = ui.shared("home.page", ""),
}

M.timeline = {
	channel_id = ui.shared("timeline.channel_id", ""),
	channel_name = ui.shared("timeline.channel_name", ""),
	channel_topic = ui.shared("timeline.channel_topic", ""),
}

M.composer = {
	open = ui.shared("composer.open", false),
	placeholder = ui.shared("composer.placeholder", ""),
	replace = ui.shared("composer.replace", { serial = 0, text = "", caret = 0 } :: ComposerReplace),
	focus = ui.shared("composer.focus", 0),
	reply = ui.shared("composer.reply", { active = false } :: ComposerReply),
	editing = ui.shared("composer.editing", false),
	files = ui.shared("composer.files", {} :: { ComposerFile }),
	suggestions = ui.shared("composer.suggestions", {} :: { ComposerSuggestion }),
	selected = ui.shared("composer.selected", 1),
	typing = ui.shared("composer.typing", ""),
	notice = ui.shared("composer.notice", { kind = "" } :: ComposerNotice),
	drop_hover = ui.shared("composer.drop_hover", false),
	limit = ui.shared("composer.limit", 2000),
}

M.user = {
	avatar = ui.shared("user.avatar", ""),
	avatar_ready = ui.shared("user.avatar_ready", false),
	panel = ui.shared("user.panel", {
		name = "",
		subtitle = "",
		emoji = "",
		emoji_image = "",
		status = "online",
		latency = "Connecting...",
	} :: UserPanel),
}

M.members = {
	heights = ui.shared("members.heights", {} :: { number }),
	window = ui.shared("members.window", { first = 0, rows = {} } :: MemberWindow),
	jump = ui.shared("members.jump", { serial = 0, offset = 0 } :: MemberJump),
}

M.window = {
	maximized = ui.shared("window.maximized", false),
}

M.shell = {
	members_visible = ui.state("members_visible", true),
}

M.settings = {
	page = ui.shared("settings.page", ""),
	values = ui.shared("settings.values", {} :: { [string]: any }),
	account = ui.shared("settings.account", nil :: SettingsAccount?),
	notes = ui.shared("settings.notes", {} :: { [string]: string }),
	about = ui.shared("settings.about", { data_dir = "", version = "" } :: SettingsAbout),
}

M.search = {
	panel = ui.shared("search.panel", {
		open = false,
		title = "",
		notice = "",
		busy = false,
		more = false,
	} :: SearchPanel),
	hits = ui.shared("search.hits", {} :: { SearchHit }),
	chips = ui.shared("search.chips", {} :: { string }),
}

return M
