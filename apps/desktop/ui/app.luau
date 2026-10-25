type Guild = { id: string, name: string, initials: string, unread: boolean, mentions: number }
type Channel = { id: string, name: string, kind: string, unread: boolean, muted: boolean, mentions: number }
type Message = { id: string, author: string, content: string, timestamp: string }

local guilds = ui.shared("guilds", {} :: { Guild })
local channels = ui.shared("channels", {} :: { Channel })
local selected = ui.shared("selected_guild", "")
local open_channel = ui.shared("open_channel", "")
local channel_name = ui.shared("channel_name", "")
local messages = ui.shared("messages", {} :: { Message })
local avatar_ready = ui.shared("avatar_ready", false)
local avatar = ui.shared("avatar", "")

ui.effect(function()
	ui.call_sync("media_reduced_motion", ui.prefers.reduced_motion:get())
end)

local palette = {
	rail = "#1e1f22",
	sidebar = "#2b2d31",
	main = "#313338",
	pill = "#313338",
	accent = "#5865f2",
	hover = "#35373c",
	text = "#f2f3f5",
	selected = "#404249",
	muted = "#949ba4",
	faint = "#80848e",
	mention = "#f23f43",
}

local glyphs = {
	text = "#",
	announcement = "!",
	voice = "~",
	stage = "*",
	forum = "=",
	thread = "-",
	other = "?",
}

local function guild_button(guild: Guild)
	local function chosen()
		return selected:get() == guild.id
	end
	return ui.row {
		w_full = true,
		justify = "center",
		items = "center",
		ui.button {
			w = 48,
			h = 48,
			rounded = function()
				return if chosen() then 16 else 24
			end,
			bg = function()
				return if chosen() then palette.accent else palette.pill
			end,
			hover = { bg = palette.accent },
			justify = "center",
			items = "center",
			cursor = "pointer",
			tooltip = guild.name,
			on_click = ui.action("select_guild", guild.id),
			ui.text { guild.initials, text_size = 14, bold = true, color = palette.text },
		},
	}
end

local readable = { text = true, announcement = true, thread = true }

local function channel_row(channel: Channel)
	if channel.kind == "category" then
		return ui.text {
			string.upper(channel.name),
			pt = 16,
			pb = 4,
			px = 8,
			text_size = 11,
			bold = true,
			color = palette.muted,
			min_w = 0,
			truncate = true,
		}
	end
	local function open()
		return open_channel:get() == channel.id
	end
	return ui.row {
		w_full = true,
		px = 8,
		py = 6,
		gap = 6,
		rounded = 4,
		items = "center",
		bg = function()
			return if open() then palette.selected else nil
		end,
		hover = { bg = palette.hover },
		cursor = if readable[channel.kind] then "pointer" else nil,
		on_click = if readable[channel.kind] then ui.action("select_channel", channel.id) else nil,
		ui.text { glyphs[channel.kind] or glyphs.other, w = 14, color = palette.faint, text_size = 15 },
		ui.text {
			channel.name,
			grow = 1,
			min_w = 0,
			truncate = true,
			text_size = 15,
			bold = channel.unread,
			color = if channel.unread then palette.text else palette.muted,
			opacity = if channel.muted then 0.5 else 1,
		},
		ui.show(
			channel.mentions > 0,
			ui.text { tostring(channel.mentions), px = 6, rounded = 8, bg = palette.mention, text_size = 11, bold = true }
		),
	}
end

local function message_row(message: Message)
	return ui.column {
		w_full = true,
		px = 16,
		py = 4,
		gap = 2,
		hover = { bg = "#2e3035" },
		ui.row {
			gap = 8,
			items = "center",
			ui.text { message.author, bold = true, text_size = 15, color = palette.text },
			ui.text { message.timestamp, text_size = 11, color = palette.faint },
		},
		ui.text { message.content, text_size = 15, color = "#dbdee1", wrap = true, w_full = true },
	}
end

local function by_id(item: { id: string })
	return item.id
end

return ui.row {
	w_full = true,
	h_full = true,
	items = "stretch",
	bg = palette.main,
	color = palette.text,

	ui.scroll {
		w = 72,
		h_full = true,
		bg = palette.rail,
		ui.each(guilds, guild_button, { key = by_id, w_full = true, gap = 8, py = 12 }),
	},

	ui.column {
		w = 240,
		h_full = true,
		bg = palette.sidebar,
		ui.row {
			h = 48,
			px = 16,
			items = "center",
			border_sides = { 0, 0, 1, 0 },
			border_color = palette.rail,
			ui.text { "{guild_name}", bold = true, text_size = 15, min_w = 0, truncate = true },
		},
		ui.scroll {
			grow = 1,
			min_h = 0,
			ui.each(channels, channel_row, { key = by_id, w_full = true, px = 8, py = 8, gap = 2 }),
		},
		ui.row {
			h = 52,
			px = 12,
			items = "center",
			bg = "#232428",
			gap = 8,
			ui.show(
				avatar_ready,
				ui.image {
					src = function()
						return avatar:get()
					end,
					w = 32,
					h = 32,
					on_hover = function(inside: boolean)
						ui.call_sync("media_hover", if inside then string.sub(avatar:get(), 6) else "")
					end,
				}
			),
			ui.text { "{status}", text_size = 13, color = palette.muted, min_w = 0, truncate = true },
		},
	},

	ui.column {
		grow = 1,
		min_w = 0,
		h_full = true,
		ui.row {
			h = 48,
			px = 16,
			items = "center",
			border_sides = { 0, 0, 1, 0 },
			border_color = palette.rail,
			ui.text {
				function()
					return if open_channel:get() == "" then "Pick a channel" else "# " .. channel_name:get()
				end,
				bold = true,
				text_size = 15,
				min_w = 0,
				truncate = true,
			},
		},
		ui.scroll {
			grow = 1,
			min_h = 0,
			follow = true,
			ui.each(messages, message_row, { key = by_id, w_full = true, py = 8 }),
		},
	},
}
