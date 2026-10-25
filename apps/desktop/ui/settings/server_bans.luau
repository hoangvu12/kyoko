local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local widgets = require("@settings/widgets")
local kit = require("@settings/server_kit")

local ROW_GAP = 8
local AVATAR = 32

local query = ui.state("server.bans_query", "")

local function bans()
	return state.server.bans:get()
end

local function row(entry: state.ServerBanRow)
	return ui.row {
		w_full = true,
		h = theme.number(theme.size.list_row_height)() + ROW_GAP,
		px = theme.size.space_12,
		gap = theme.size.space_12,
		items = "center",
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.divider,
		avatar { src = entry.avatar, size = AVATAR },
		ui.column {
			grow = 1,
			min_w = 0,
			widgets.label(entry.name, theme.size.font_md, theme.color.header_primary, {
				weight = theme.weight.semibold,
				literal = true,
				truncate = true,
			}),
			widgets.label(entry.reason, theme.size.font_sm, theme.color.text_muted, { literal = true, truncate = true }),
		},
		kit.button {
			id = "ban_revoke_" .. entry.id,
			text = "Revoke Ban",
			tone = "outline",
			on_click = function()
				actions.server_action("unban", { id = entry.id, name = entry.name })
			end,
		},
	}
end

return function()
	ui.effect(function()
		actions.server_action("bans_search", query:get())
	end)
	return ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		kit.title("Bans"),
		ui.div {
			w_full = true,
			shrink = 0,
			pb = theme.size.space_16,
			kit.input(query, "Search by username or ID"),
		},
		kit.caption(function()
			local value = bans()
			local count = if value then #value.rows else 0
			return string.format("%d %s", count, if count == 1 then "ban" else "bans")
		end),
		kit.rows(bans, row, kit.empty("ban", "Nobody is banned from this server.")),
		ui.show(function()
			local value = bans()
			return value ~= nil and value.has_more
		end, ui.row {
			w_full = true,
			justify = "center",
			pb = theme.size.space_16,
			kit.button {
				text = "Load more",
				tone = "outline",
				on_click = function()
					actions.server_action("bans_more")
				end,
			},
		}),
	}
end
