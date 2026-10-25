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
export type Channel = { id: string, name: string, kind: string, unread: boolean, muted: boolean, mentions: number }

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
	channels = ui.shared("sidebar.channels", {} :: { Channel }),
	guild_id = ui.shared("sidebar.guild_id", ""),
	guild_name = ui.shared("sidebar.guild_name", ""),
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
