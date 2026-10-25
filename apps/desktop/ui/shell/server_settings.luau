local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local widgets = require("@settings/widgets")
local frame = require("@settings/frame")
local overview = require("@settings/server_overview")
local roles = require("@settings/server_roles")
local members = require("@settings/server_members")
local bans = require("@settings/server_bans")
local audit = require("@settings/server_audit")
local emoji = require("@settings/server_emoji")
local stickers = require("@settings/server_stickers")
local invites = require("@settings/server_invites")

local TOP = 60
local NAV_PAD = 8
local CONTENT_ROOM = 128
local COLUMN_ROOM = 96

local BUILDERS: { [string]: () -> any } = {
	overview = overview,
	roles = roles,
	members = members,
	bans = bans,
	["audit-log"] = audit,
	emoji = emoji,
	stickers = stickers,
	invites = invites,
}

local function page_key(): string
	return state.server.page:get()
end

local function nav()
	return ui.presence(function()
		return state.server.guild:get()
	end, function(guild: state.ServerGuild)
		local items = {
			ui.div {
				px = NAV_PAD,
				pb = theme.size.space_16,
				widgets.label(guild.name, theme.size.font_md, theme.color.header_primary, {
					weight = theme.weight.bold,
					literal = true,
					truncate = true,
				}),
			},
		}
		for _, group in guild.groups do
			local entries = {
				ui.div {
					px = NAV_PAD,
					pb = theme.size.space_4,
					widgets.label(string.upper(group.title), theme.size.font_tiny, theme.color.header_secondary, {
						weight = theme.weight.bold,
						literal = true,
					}),
				},
			}
			for _, page in group.pages do
				table.insert(
					entries,
					frame.nav_item {
						title = page.title,
						icon = page.icon,
						selected = function()
							return page_key() == page.key
						end,
						on_click = function()
							actions.open_server_settings(page.key)
						end,
						height = theme.number(theme.size.control_height)(),
						pad = NAV_PAD,
						glyph = theme.number(theme.size.glyph_md)(),
						id = "server_nav_" .. page.key,
					}
				)
			end
			table.insert(
				items,
				ui.column {
					w_full = true,
					gap = theme.size.space_2,
					pb = theme.size.space_16,
					table.unpack(entries),
				}
			)
		end
		return ui.column {
			w = theme.size.settings_nav_width,
			h_full = true,
			shrink = 0,
			pt = TOP,
			pr = theme.size.space_16,
			table.unpack(items),
		}
	end)
end

local function content()
	return ui.presence(function()
		local key = page_key()
		return if key ~= "" then key else nil
	end, function(key: string)
		local build = BUILDERS[key]
		return if build then build() else ui.div {}
	end)
end

local function view()
	local root = ui.ref()
	local nav_width = theme.number(theme.size.settings_nav_width)
	local content_width = theme.number(theme.size.settings_content_width)
	local function column_width(): number
		local width = root.width or 0
		return math.max(0, math.min(content_width(), width - nav_width() - CONTENT_ROOM))
	end
	local function left(): number
		local width = root.width or 0
		return math.max(0, (width - nav_width() - column_width() - COLUMN_ROOM) / 2) + nav_width()
	end
	return ui.row {
		ref = root,
		absolute = true,
		top = theme.number(theme.size.title_bar_height),
		left = 0,
		right = 0,
		bottom = 0,
		bg = theme.color.background_primary,
		color = theme.color.text_normal,
		focusable = true,
		on_mount = function(element: any)
			element:focus()
		end,
		on_key = function(event: any): boolean?
			if event.key == "Escape" then
				actions.close_server_settings()
				return true
			end
			return false
		end,
		ui.row {
			w = left,
			h_full = true,
			shrink = 0,
			justify = "end",
			bg = theme.color.background_secondary,
			nav(),
		},
		ui.div {
			relative = true,
			grow = 1,
			min_w = 0,
			h_full = true,
			overflow = "hidden",
			ui.column {
				w = column_width,
				h_full = true,
				pt = TOP,
				pl = theme.size.space_32,
				pr = 64,
				content(),
			},
			frame.close_button { on_close = actions.close_server_settings, esc = false, top = TOP },
		},
	}
end

return function()
	return ui.show(function()
		return page_key() ~= ""
	end, view())
end
