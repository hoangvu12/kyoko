local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local widgets = require("@settings/widgets")
local kit = require("@settings/server_kit")

local AVATAR = 24

local function invites()
	return state.server.invites:get()
end

local function row(entry: state.ServerInviteRow)
	local cell = theme.number(theme.size.manage_grid_cell)()
	return ui.row {
		w_full = true,
		h = theme.size.list_row_height,
		px = theme.size.space_12,
		gap = theme.size.space_12,
		items = "center",
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.divider,
		ui.row {
			w = cell * 2,
			shrink = 0,
			gap = theme.size.space_8,
			items = "center",
			if entry.inviter ~= "" then avatar { src = entry.avatar, size = AVATAR } else nil,
			widgets.label(entry.inviter, theme.size.font_sm, theme.color.text_normal, { literal = true, truncate = true }),
		},
		widgets.label(entry.id, theme.size.font_md, theme.color.header_primary, {
			weight = theme.weight.semibold,
			literal = true,
			truncate = true,
			grow = 1,
			min_w = 0,
		}),
		widgets.label(entry.channel, theme.size.font_sm, theme.color.text_muted, {
			literal = true,
			truncate = true,
			w = cell,
			shrink = 0,
		}),
		widgets.label(entry.uses, theme.size.font_sm, theme.color.text_muted, {
			literal = true,
			w = theme.size.picker_rail,
			shrink = 0,
		}),
		widgets.label(entry.expires, theme.size.font_sm, theme.color.text_muted, {
			literal = true,
			truncate = true,
			w = cell,
			shrink = 0,
		}),
		if entry.temporary then kit.badge("Temporary", theme.color.warning, "") else nil,
		kit.icon_button {
			name = "copy",
			tooltip = "Copy",
			on_click = function()
				actions.server_action("invite_copy", entry.id)
			end,
		},
		kit.icon_button {
			name = "trash",
			tooltip = "Revoke Invite",
			tint = theme.color.danger,
			on_click = function()
				actions.server_action("invite_revoke", entry.id)
			end,
		},
	}
end

return function()
	return ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		kit.title("Invites"),
		kit.caption(function()
			local value = invites()
			local count = if value then #value.rows else 0
			return string.format("%d %s", count, if count == 1 then "invite" else "invites")
		end),
		kit.rows(invites, row, kit.empty("mail", "There are no active invites.")),
	}
end
