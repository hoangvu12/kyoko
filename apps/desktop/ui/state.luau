export type Guild = { id: string, name: string, initials: string, unread: boolean, mentions: number }
export type Channel = { id: string, name: string, kind: string, unread: boolean, muted: boolean, mentions: number }

local M = {}

M.session = {
	status = ui.shared("session.status", "Connecting"),
}

M.rail = {
	guilds = ui.shared("rail.guilds", {} :: { Guild }),
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
