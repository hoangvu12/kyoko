type Guild = { id: string, name: string, initials: string, unread: boolean, mentions: number }
type Channel = { id: string, name: string, kind: string, unread: boolean, muted: boolean, mentions: number }

local guilds = ui.shared("guilds", {} :: { Guild })
local channels = ui.shared("channels", {} :: { Channel })
local selected = ui.shared("selected_guild", "")

local palette = {
	rail = "#1e1f22",
	sidebar = "#2b2d31",
	main = "#313338",
	pill = "#313338",
	accent = "#5865f2",
	hover = "#35373c",
	text = "#f2f3f5",
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
	return ui.row {
		w_full = true,
		px = 8,
		py = 6,
		gap = 6,
		rounded = 4,
		items = "center",
		hover = { bg = palette.hover },
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
			ui.text { "{status}", text_size = 13, color = palette.muted, min_w = 0, truncate = true },
		},
	},

	ui.column {
		grow = 1,
		justify = "center",
		items = "center",
		ui.text { "Pick a channel", color = palette.faint, text_size = 15 },
	},
}
