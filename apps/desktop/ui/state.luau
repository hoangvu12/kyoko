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

M.timeline = {
	channel_id = ui.shared("timeline.channel_id", ""),
	channel_name = ui.shared("timeline.channel_name", ""),
	channel_topic = ui.shared("timeline.channel_topic", ""),
}

M.user = {
	avatar = ui.shared("user.avatar", ""),
	avatar_ready = ui.shared("user.avatar_ready", false),
}

M.window = {
	maximized = ui.shared("window.maximized", false),
}

M.shell = {
	members_visible = ui.state("members_visible", true),
}

return M
