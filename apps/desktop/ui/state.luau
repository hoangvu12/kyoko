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
export type ConnectionBanner = { shown: boolean, state: string, text: string, tone: string, retry: boolean }
export type FriendRow = {
	kind: string,
	key: string,
	title: string?,
	id: string,
	name: string,
	username: string,
	avatar: string,
	status: string,
	subtitle: string,
	relation: string,
}
export type FriendAdd = { phase: string, text: string }
export type ComposerReplace = { serial: number, text: string, caret: number }
export type ComposerReply = { active: boolean, name: string?, ping: boolean?, own: boolean? }
export type ComposerFile = { id: string, name: string, size: string, image: string }
export type ComposerSuggestion = { key: string, label: string, detail: string, kind: string, image: string, glyph: string, icon: string, svg: string? }
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
export type PluginChoice = { value: string, label: string }
export type PluginSetting = {
	key: string,
	type: string,
	label: string,
	description: string,
	value: any,
	min: number,
	max: number,
	step: number,
	multiline: boolean,
	options: { PluginChoice },
	fields: { PluginSetting },
	item: string,
	rows: number,
}
export type PluginPermission = { name: string, text: string, sensitive: boolean, reserved: boolean }
export type Plugin = {
	id: string,
	name: string,
	version: string,
	description: string,
	authors: string,
	origin: string,
	origin_label: string,
	status: string,
	status_label: string,
	enabled: boolean,
	needs_approval: boolean,
	notice: string,
	error: string,
	settings: { PluginSetting },
	permissions: { PluginPermission },
}
export type PluginNotice = { plugin: string, text: string }
export type ThemeEntry = {
	id: string,
	source: string,
	name: string,
	author: string,
	description: string,
	version: string,
	enabled: boolean,
	error: string,
	note: string,
	scheme: string,
	swatch: { string },
}
export type UserPanel = {
	name: string,
	subtitle: string,
	emoji: string,
	emoji_image: string,
	emoji_svg: string?,
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
export type ThreadPanel = {
	open: boolean,
	id: string,
	name: string,
	members: number,
	archived: boolean,
	joined: boolean,
	can_manage: boolean,
}
export type ForumView = { open: boolean, id: string, sort: string, gallery: boolean }
export type ForumTag = { key: string, id: string, label: string, active: boolean }
export type ForumBadge = { key: string, label: string, moderated: boolean }
export type ForumCard = {
	key: string,
	id: string,
	title: string,
	excerpt: string,
	tags: { ForumBadge },
	author: string,
	avatar: string,
	replies: string,
	activity: string,
	pinned: boolean,
	archived: boolean,
}

export type ServerPageEntry = { key: string, title: string, icon: string }
export type ServerGroup = { title: string, pages: { ServerPageEntry } }
export type ServerGuild = { id: string, name: string, groups: { ServerGroup } }
export type ServerOverview = { name: string, description: string, icon: string, pending: boolean }
export type ServerRoleRow = {
	id: string,
	name: string,
	color: string,
	members: number,
	everyone: boolean,
	movable: boolean,
}
export type ServerRole = {
	id: string,
	name: string,
	title: string,
	color: string,
	hoist: boolean,
	mentionable: boolean,
	everyone: boolean,
	deletable: boolean,
	granted: { number },
}
export type ServerRight = { index: number, name: string, hint: string }
export type ServerRightGroup = { title: string, rights: { ServerRight } }
export type ServerChip = { name: string, color: string, tint: string }
export type ServerMemberRow = {
	id: string,
	name: string,
	username: string,
	color: string,
	avatar: string,
	roles: { ServerChip },
}
export type ServerUpload = { serial: number, name: string, preview: string }
export type ServerEmojiRow = { id: string, name: string, image: string, uploader: string, avatar: string }
export type ServerStickerRow = { id: string, name: string, tags: string, image: string }
export type ServerInviteRow = {
	id: string,
	inviter: string,
	avatar: string,
	channel: string,
	uses: string,
	expires: string,
	temporary: boolean,
}
export type ServerBanRow = { id: string, name: string, reason: string, avatar: string }
export type ServerAuditChange = { key: string, old: string, new: string }
export type ServerAuditRow = {
	id: string,
	sentence: string,
	when: string,
	avatar: string,
	changes: { ServerAuditChange },
	reason: string,
}

local M = {}

M.session = {
	status = ui.shared("session.status", "Connecting"),
	storage_warning = ui.shared("session.storage_warning", ""),
	connection = ui.shared(
		"session.connection",
		{ shown = false, state = "connecting", text = "", tone = "warning", retry = false } :: ConnectionBanner
	),
}

M.login = {
	screen = ui.shared("login.screen", "login"),
	acknowledged = ui.shared("login.acknowledged", false),
	checking = ui.shared("login.checking", false),
	message = ui.shared("login.message", ""),
	error = ui.shared("login.error", false),
	unsaved = ui.shared("login.unsaved", false),
	adding = ui.shared("login.adding", false),
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

M.friends = {
	tab = ui.shared("friends.tab", "online"),
	rows = ui.shared("friends.rows", {} :: { FriendRow }),
	empty = ui.shared("friends.empty", ""),
	pending = ui.shared("friends.pending", 0),
	active = ui.shared("friends.active", {} :: { FriendRow }),
	add = ui.shared("friends.add", { phase = "idle", text = "" } :: FriendAdd),
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
		emoji_svg = "",
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

M.plugins = {
	list = ui.shared("plugins.list", {} :: { Plugin }),
	notice = ui.shared("plugins.notice", nil :: PluginNotice?),
	folder = ui.shared("plugins.folder", ""),
	slots = ui.shared("plugins.slots", {} :: { [string]: { string } }),
}

M.themes = {
	list = ui.shared("themes.list", {} :: { ThemeEntry }),
	links = ui.shared("themes.links", ""),
	folder = ui.shared("themes.folder", ""),
}

M.timeline.channel_icon = ui.shared("timeline.channel_icon", "hash")

M.thread_composer = {
	open = ui.shared("thread_composer.open", false),
	placeholder = ui.shared("thread_composer.placeholder", ""),
	replace = ui.shared("thread_composer.replace", { serial = 0, text = "", caret = 0 } :: ComposerReplace),
	focus = ui.shared("thread_composer.focus", 0),
	reply = ui.shared("thread_composer.reply", { active = false } :: ComposerReply),
	editing = ui.shared("thread_composer.editing", false),
	files = ui.shared("thread_composer.files", {} :: { ComposerFile }),
	suggestions = ui.shared("thread_composer.suggestions", {} :: { ComposerSuggestion }),
	selected = ui.shared("thread_composer.selected", 1),
	typing = ui.shared("thread_composer.typing", ""),
	notice = ui.shared("thread_composer.notice", { kind = "" } :: ComposerNotice),
	drop_hover = ui.shared("thread_composer.drop_hover", false),
	limit = ui.shared("thread_composer.limit", 2000),
}

M.threads = {
	panel = ui.shared("threads.panel", {
		open = false,
		id = "",
		name = "",
		members = 0,
		archived = false,
		joined = true,
		can_manage = false,
	} :: ThreadPanel),
}

M.forum = {
	view = ui.shared("forum.view", { open = false, id = "", sort = "Recently Active", gallery = false } :: ForumView),
	tags = ui.shared("forum.tags", {} :: { ForumTag }),
	cards = ui.shared("forum.cards", {} :: { ForumCard }),
}

M.server = {
	page = ui.shared("server.page", ""),
	guild = ui.shared("server.guild", nil :: ServerGuild?),
	overview = ui.shared("server.overview", nil :: ServerOverview?),
	roles = ui.shared("server.roles", nil :: { count: number, rows: { ServerRoleRow } }?),
	role = ui.shared("server.role", nil :: ServerRole?),
	rights = ui.shared("server.rights", {} :: { ServerRightGroup }),
	members = ui.shared("server.members", nil :: { total: number, rows: { ServerMemberRow }, actions: boolean }?),
	bans = ui.shared("server.bans", nil :: { loaded: boolean, has_more: boolean, rows: { ServerBanRow } }?),
	audit = ui.shared("server.audit", nil :: {
		loaded: boolean,
		has_more: boolean,
		filter: number,
		rows: { ServerAuditRow },
	}?),
	audit_filters = ui.shared("server.audit_filters", {} :: { string }),
	emoji = ui.shared("server.emoji", nil :: {
		still: number,
		animated: number,
		slots: number,
		rows: { ServerEmojiRow },
		upload: ServerUpload?,
	}?),
	stickers = ui.shared("server.stickers", nil :: {
		count: number,
		slots: number,
		rows: { ServerStickerRow },
		upload: ServerUpload?,
	}?),
	invites = ui.shared("server.invites", nil :: { rows: { ServerInviteRow } }?),
}

return M
