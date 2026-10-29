export type RailVoice = { key: string, avatar: string }
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
	voice: { RailVoice }?,
	voice_more: number?,
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
export type LoginQr = {
	stage: string,
	image: string?,
	username: string?,
	avatar: string?,
	message: string?,
	error: boolean?,
	web: boolean?,
}
export type LoginWeb = {
	open: boolean,
	signing_in: boolean,
	loaded: boolean,
	host: string,
	path: string,
	secure: boolean,
	message: string,
	error: boolean,
}
export type LoginPassword = {
	stage: string,
	problem: string,
	method: string?,
	phone: string?,
	awaits_text: boolean?,
	others: { string }?,
	title: string?,
	message: string?,
}
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
export type SidebarBadge = { unread: boolean, mentions: number }
export type SidebarRow = {
	kind: string,
	key: string,
	id: string,
	name: string,
	collapsed: boolean?,
	type: string?,
	icon: string?,
	muted: boolean,
	locked: boolean?,
	joined: boolean?,
	selectable: boolean?,
	manageable: boolean?,
	hidden: boolean?,
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
	in_call: boolean?,
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
	details: string?,
	state: string?,
}
export type DmProfile = {
	open: boolean,
	id: string,
	name: string,
	username: string,
	tag: string,
	tag_badge: string,
	avatar: string,
	banner: string,
	accent: string,
	status: string,
	bio: string,
	since: string,
	servers: string,
	friends: string,
	server_icons: { DmMutual },
	friend_icons: { DmMutual },
	friend: string,
	loading: boolean,
	theme_top: string,
	theme_bottom: string,
	effect: boolean,
}
export type DmMutual = {
	key: string,
	src: string,
	initial: string,
}
export type DmMember = {
	key: string,
	id: string,
	name: string,
	avatar: string,
	status: string,
	activity: string,
	owner: boolean,
}
export type FriendAdd = { phase: string, text: string }
export type ComposerReplace = { serial: number, text: string, caret: number }
export type ComposerReply = { active: boolean, name: string?, ping: boolean?, own: boolean? }
export type ComposerFile = { id: string, name: string, size: string, image: string, kind: string, spoiler: boolean }
export type ComposerSuggestion = { key: string, label: string, detail: string, kind: string, image: string, glyph: string, icon: string, svg: string?, lock: string? }
export type SuggestionHeading = { title: string, query: string }
export type ComposerNotice = { kind: string, text: string?, confirmable: boolean?, nonce: string? }
export type CommandVisual = { kind: string, letter: string?, hue: number?, image: string? }
export type CommandPickerState = { open: boolean, items: { any }?, rail: { any }?, selected: number?, section: number? }
export type CommandState = { active: boolean, serial: number?, app: CommandVisual?, name: string?, focused: number?, fields: { any }?, hint: { name: string, description: string, error: boolean }?, optional_count: number?, optional_open: boolean?, optional_selected: number?, optional: { any }?, choices: any?, wants_file: boolean? }
export type MemberWindow = {
	first: number,
	rows: { any },
}
export type MemberJump = {
	serial: number,
	offset: number,
}
export type SettingsAccount = { name: string, username: string, email: string, phone: string, accent: string, status: string, avatar: string, shares_activity: boolean? }
export type SettingsAbout = { data_dir: string, version: string, platform: string? }
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
export type SearchHit = {
	key: string,
	id: string,
	author: string,
	avatar: string,
	time: string,
	place: string,
	text: string,
	runs: { { text: string, mark: boolean } }?,
	group: string,
	group_icon: string,
}
export type SearchPanel = {
	open: boolean,
	title: string,
	notice: string,
	busy: boolean,
	more: boolean,
	tab: string,
	oldest: boolean,
	page: number,
	pages: number,
}
export type ThreadPanel = {
	open: boolean,
	id: string,
	name: string,
	members: number,
	archived: boolean,
	joined: boolean,
	can_manage: boolean,
	creating: boolean,
	serial: number,
	draft_name: string,
	starter_author: string,
	starter_time: string,
	starter_text: string,
	starter_avatar: string,
	can_private: boolean,
	post: boolean,
	tags: { string },
}
export type ForumView = {
	open: boolean,
	id: string,
	sort: string,
	gallery: boolean,
	composing: boolean,
	serial: number,
	draft_title: string,
	require_tag: boolean,
}
export type ForumFile = { key: string, name: string, size: string }
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
	count: string,
	activity: string,
	pinned: boolean,
	archived: boolean,
}

export type ServerPageEntry = { key: string, title: string, icon: string }
export type ServerGroup = { title: string, pages: { ServerPageEntry } }
export type ServerGuild = { id: string, name: string, icon: string?, acronym: string?, groups: { ServerGroup } }
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
	since: string?,
}
export type ServerUpload = { serial: number, name: string, preview: string }
export type ServerEmojiRow = {
	id: string,
	name: string,
	image: string,
	uploader: string,
	avatar: string,
	animated: boolean?,
	color: string?,
}
export type ServerStickerRow = { id: string, name: string, tags: string, image: string }
export type ServerInviteRow = {
	id: string,
	inviter: string,
	avatar: string,
	channel: string,
	uses: string,
	expires: string,
	temporary: boolean,
	color: string?,
}
export type ServerBanRow = { id: string, name: string, username: string?, reason: string, avatar: string }
export type ServerAuditChange = { key: string, old: string, new: string }
export type ServerAuditRow = {
	id: string,
	sentence: string,
	when: string,
	avatar: string,
	changes: { ServerAuditChange },
	reason: string,
	parts: { { text: string, strong: boolean } }?,
	icon: string?,
	tone: string?,
}
export type ServerAuditPerson = { id: string, name: string }

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
	qr = ui.shared("login.qr", nil :: LoginQr?),
	web = ui.shared("login.web", nil :: LoginWeb?),
	password = ui.shared("login.password", nil :: LoginPassword?),
	mode = ui.shared("login.mode", "discord"),
	unsaved_name = ui.shared("login.unsaved_name", ""),
}

M.rail = {
	items = ui.shared("rail.items", {} :: { RailItem }),
	home_mentions = ui.shared("rail.home_mentions", 0),
	open_folders = ui.shared("rail.open_folders", {} :: { string }),
}

M.sidebar = {
	rows = ui.shared("sidebar.rows", {} :: { SidebarRow }),
	badges = ui.shared("sidebar.badges", {} :: { [string]: SidebarBadge }),
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
	profile = ui.shared("dms.profile", {
		open = false,
		id = "",
		name = "",
		username = "",
		tag = "",
		tag_badge = "",
		avatar = "",
		banner = "",
		accent = "",
		status = "offline",
		bio = "",
		since = "",
		servers = "",
		friends = "",
		server_icons = {},
		friend_icons = {},
		friend = "",
		loading = false,
		theme_top = "",
		theme_bottom = "",
		effect = false,
	} :: DmProfile),
	members = ui.shared("dms.members", {} :: { DmMember }),
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
	heading = ui.shared("composer.heading", { title = "", query = "" } :: SuggestionHeading),
	selected = ui.shared("composer.selected", 1),
	typing = ui.shared("composer.typing", ""),
	notice = ui.shared("composer.notice", { kind = "" } :: ComposerNotice),
	drop_hover = ui.shared("composer.drop_hover", false),
	limit = ui.shared("composer.limit", 2000),
	pick = ui.shared("composer.pick", 0),
	picker = ui.shared("composer.picker_state", { open = false } :: CommandPickerState),
	command = ui.shared("composer.command", { active = false } :: CommandState),
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
	dm_profile_visible = ui.state("dm_profile_visible", true),
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
		tab = "messages",
		oldest = false,
		page = 1,
		pages = 1,
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
	heading = ui.shared("thread_composer.heading", { title = "", query = "" } :: SuggestionHeading),
	selected = ui.shared("thread_composer.selected", 1),
	typing = ui.shared("thread_composer.typing", ""),
	notice = ui.shared("thread_composer.notice", { kind = "" } :: ComposerNotice),
	drop_hover = ui.shared("thread_composer.drop_hover", false),
	limit = ui.shared("thread_composer.limit", 2000),
	pick = ui.shared("thread_composer.pick", 0),
	picker = ui.shared("thread_composer.picker_state", { open = false } :: CommandPickerState),
	command = ui.shared("thread_composer.command", { active = false } :: CommandState),
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
		creating = false,
		serial = 0,
		draft_name = "",
		starter_author = "",
		starter_time = "",
		starter_text = "",
		starter_avatar = "",
		can_private = false,
		post = false,
		tags = {},
	} :: ThreadPanel),
}

M.forum = {
	view = ui.shared("forum.view", {
		open = false,
		id = "",
		sort = "Recently Active",
		gallery = false,
		composing = false,
		serial = 0,
		draft_title = "",
		require_tag = false,
	} :: ForumView),
	files = ui.shared("forum.files", {} :: { ForumFile }),
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
	members = ui.shared("server.members", nil :: {
		total: number,
		rows: { ServerMemberRow },
		actions: boolean,
		shown: number?,
		count: number?,
		sort: number?,
	}?),
	member_sorts = ui.shared("server.member_sorts", {} :: { string }),
	bans = ui.shared("server.bans", nil :: { loaded: boolean, has_more: boolean, rows: { ServerBanRow } }?),
	audit = ui.shared("server.audit", nil :: {
		loaded: boolean,
		has_more: boolean,
		filter: number,
		rows: { ServerAuditRow },
		people: { ServerAuditPerson }?,
		person: number?,
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
		tier: number?,
		rows: { ServerStickerRow },
		upload: ServerUpload?,
	}?),
	invites = ui.shared("server.invites", nil :: { rows: { ServerInviteRow } }?),
}

export type ChannelInfo = { id: string, name: string, label: string, icon: string, pages: { ServerPageEntry } }
export type ChannelOverview = {
	id: string,
	name: string,
	topic: string,
	slowmode: number,
	visibility: string,
	nsfw: boolean,
	announcement: boolean,
	archive: number,
	bitrate: number,
	bitrate_min: number,
	bitrate_max: number,
	user_limit: number,
	user_limit_max: number,
	video_quality: number,
	region: number,
	regions: { string },
	category: number,
	categories: { string },
	audio: boolean,
	has_topic: boolean,
	archives: boolean,
	announces: boolean,
}
export type ChannelOverwriteRow = {
	key: string,
	id: string,
	name: string,
	color: string,
	member: boolean,
	everyone: boolean,
	editable: boolean,
}
export type ChannelOverwrite = {
	id: string,
	name: string,
	color: string,
	member: boolean,
	everyone: boolean,
	editable: boolean,
	choices: { [string]: number },
	serial: string,
}
export type ChannelCandidate = { id: string, name: string, color: string, member: boolean }

M.channel = {
	page = ui.shared("channel.page", ""),
	info = ui.shared("channel.info", nil :: ChannelInfo?),
	overview = ui.shared("channel.overview", nil :: ChannelOverview?),
	overwrites = ui.shared("channel.overwrites", nil :: {
		rows: { ChannelOverwriteRow },
		groups: { ServerRightGroup },
		editing: string,
	}?),
	overwrite = ui.shared("channel.overwrite", nil :: ChannelOverwrite?),
	candidates = ui.shared("channel.candidates", {} :: { ChannelCandidate }),
	invites = ui.shared("channel.invites", nil :: { loaded: boolean, rows: { ServerInviteRow } }?),
	choices = ui.shared("channel.choices", { slowmode = {}, archive = {} } :: { slowmode: { string }, archive: { string } }),
}

export type HiddenOverwrite = { key: string, name: string, member: boolean, color: string }
export type HiddenView = {
	open: boolean,
	id: string,
	name: string,
	kind: string,
	icon: string,
	topic: string,
	activity: string,
	created: string,
	nsfw: boolean,
	slowmode: string,
	allowed: { HiddenOverwrite },
	denied: { HiddenOverwrite },
}

M.hidden = {
	view = ui.shared("hidden.view", {
		open = false,
		id = "",
		name = "",
		kind = "",
		icon = "hash",
		topic = "",
		activity = "",
		created = "",
		nsfw = false,
		slowmode = "",
		allowed = {},
		denied = {},
	} :: HiddenView),
}

export type VoicePanel = {
	open: boolean,
	channel_id: string,
	status: string,
	connected: boolean,
	place: string,
}
export type CallTile = {
	key: string,
	id: string,
	name: string,
	avatar: string,
	me: boolean,
	muted: boolean,
	deafened: boolean,
	video: string,
	live: boolean,
	volume: number,
	local_muted: boolean,
	ringing: boolean?,
	stream: boolean,
	stream_key: string,
	watching: boolean,
	paused: boolean,
}
export type VoiceCall = {
	open: boolean,
	channel_id: string,
	name: string,
	connected: boolean,
	tiles: { CallTile },
}
export type VoiceControls = {
	muted: boolean,
	deafened: boolean,
	server_muted: boolean,
	server_deafened: boolean,
	camera: boolean,
	live: boolean,
}

export type AudioDevice = { id: string, name: string }

export type Camera = { id: string, name: string }

export type AudioDevices = { inputs: { AudioDevice }, outputs: { AudioDevice } }

export type MicTest = { running: boolean, level: number }

export type IncomingCall = { open: boolean, channel_id: string, name: string, avatar: string, group: boolean }

export type CallPerson = { id: string, name: string, avatar: string, ringing: boolean }

export type DmCall = { shown: boolean, channel_id: string, joined: boolean, name: string, people: { CallPerson } }

export type ShareSource = { id: string, kind: string, name: string, size: string }

export type ShareSources = { open: boolean, loading: boolean, items: { ShareSource } }

export type ScreenPreview = {
	id: string,
	name: string,
	resolution: string,
	fps: number,
	width: number,
	height: number,
	running: boolean,
}

M.voice = {
	panel = ui.shared("voice.panel", {
		open = false,
		channel_id = "",
		status = "",
		connected = false,
		place = "",
	} :: VoicePanel),
	controls = ui.shared("voice.controls", {
		muted = false,
		deafened = false,
		server_muted = false,
		server_deafened = false,
		camera = false,
		live = false,
	} :: VoiceControls),
	call = ui.shared("voice.call", {
		open = false,
		channel_id = "",
		name = "",
		connected = false,
		tiles = {},
	} :: VoiceCall),
	speaking = ui.shared("voice.speaking", {} :: { [string]: boolean }),
	devices = ui.shared("voice.devices", { inputs = {}, outputs = {} } :: AudioDevices),
	mic_test = ui.shared("voice.mic_test", { running = false, level = 0 } :: MicTest),
	video_available = ui.shared("voice.video_available", true),
	cameras = ui.shared("voice.cameras", {} :: { Camera }),
	camera_preview = ui.shared("voice.camera_preview", false),
	screens = ui.shared("voice.screens", { open = false, loading = false, items = {} } :: ShareSources),
	screen_preview = ui.shared("voice.screen_preview", {
		id = "",
		name = "",
		resolution = "720",
		fps = 30,
		width = 0,
		height = 0,
		running = false,
	} :: ScreenPreview),
	incoming = ui.shared("voice.incoming", {
		open = false,
		channel_id = "",
		name = "",
		avatar = "",
		group = false,
	} :: IncomingCall),
	dm_call = ui.shared("voice.dm_call", {
		shown = false,
		channel_id = "",
		joined = false,
		name = "",
		people = {},
	} :: DmCall),
}

export type SideButton = { binding: string, serial: number }

M.keybinds = {
	side_button = ui.shared("keybinds.side_button", nil :: SideButton?),
}

return M
