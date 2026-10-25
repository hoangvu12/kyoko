local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local widgets = require("@settings/widgets")
local kit = require("@settings/server_kit")

local ROW_GAP = 8
local NAME_COLUMN = 216
local AVATAR = 32

local query = ui.state("server.members_query", "")

local function members()
	return state.server.members:get()
end

local function row(entry: state.ServerMemberRow)
	local badges = {}
	for _, chip in entry.roles do
		table.insert(badges, kit.badge(chip.name, chip.color, chip.tint))
	end
	return ui.row {
		w_full = true,
		h = theme.number(theme.size.list_row_height)() + ROW_GAP,
		px = theme.size.space_12,
		gap = theme.size.space_12,
		items = "center",
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.divider,
		context_menu = function(event: any)
			actions.server_action("member_menu", { id = entry.id, x = event.x, y = event.y })
			return nil
		end,
		avatar { src = entry.avatar, size = AVATAR },
		ui.column {
			w = NAME_COLUMN,
			shrink = 0,
			min_w = 0,
			widgets.label(entry.name, theme.size.font_md, if entry.color ~= "" then entry.color else theme.color.header_primary, {
				weight = theme.weight.semibold,
				literal = true,
				truncate = true,
			}),
			widgets.label(entry.username, theme.size.font_sm, theme.color.text_muted, { literal = true, truncate = true }),
		},
		ui.row {
			grow = 1,
			min_w = 0,
			flex_wrap = true,
			gap = theme.size.space_4,
			table.unpack(badges),
		},
		ui.show(function()
			local value = members()
			return value ~= nil and value.actions
		end, ui.button {
			w = 28,
			h = 28,
			shrink = 0,
			p = 0,
			border = 0,
			focus = { border = 0 },
			rounded = theme.size.radius_sm,
			justify = "center",
			items = "center",
			cursor = "pointer",
			tooltip = "More",
			bg = "#00000000",
			on_press = function(event: any)
				actions.server_action("member_menu", { id = entry.id, x = event.x, y = event.y })
			end,
			require("@components/icon") { name = "more", size = 16, color = theme.color.interactive_normal },
		}),
	}
end

return function()
	ui.effect(function()
		actions.server_action("members_search", query:get())
	end)
	return ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		kit.title("Members"),
		ui.div {
			w_full = true,
			shrink = 0,
			pb = theme.size.space_16,
			kit.input(query, "Search members", {
				on_key = function(event: any): boolean?
					if event.key == "Enter" then
						actions.server_action("members_remote", query:get())
						return true
					end
					return false
				end,
			}),
		},
		kit.caption(function()
			local value = members()
			local total = if value then value.total else 0
			return string.format("%d %s", total, if total == 1 then "member" else "members")
		end),
		kit.rows(members, row, kit.empty("members", "No members match your search.")),
	}
end
