local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local kit = require("./kit")

local function n(token: string): number
	return theme.number(token)()
end

local function invites()
	return state.channel.invites:get()
end

local COLUMNS: { kit.Column } = {
	{ title = "Inviter", grow = 1.4 },
	{ title = "Invite code", grow = 1 },
	{ title = "Uses", w = function()
		return n(theme.size.space_56)
	end },
	{ title = "Expires", grow = 1 },
}

local function row(entry: state.ServerInviteRow)
	return kit.table_row {
		id = "channel_invite_row_" .. entry.id,
		columns = COLUMNS,
		cells = {
			kit.person(entry.avatar, if entry.inviter ~= "" then entry.inviter else "Unknown", entry.color),
			kit.text(entry.id, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_3, {
				font = "mono",
				truncate = true,
				min_w = 0,
			}),
			kit.text(entry.uses, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_3),
			kit.text(
				if entry.temporary then entry.expires .. " · Temporary" else entry.expires,
				theme.size.font_size_sm,
				theme.size.line_height_sm,
				theme.color.text_3,
				{ truncate = true, min_w = 0 }
			),
		},
		actions = function(hovered: () -> boolean)
			return kit.row_actions(
				hovered,
				kit.round_button {
					id = "channel_invite_copy_" .. entry.id,
					name = "copy",
					tooltip = "Copy",
					on_click = function()
						actions.channel_action("invite_copy", entry.id)
					end,
				},
				kit.round_button {
					id = "channel_invite_revoke_" .. entry.id,
					name = "close",
					tooltip = "Revoke Invite",
					tint = theme.color.red_2,
					on_click = function()
						actions.channel_action("invite_revoke", entry.id)
					end,
				}
			)
		end,
	}
end

local function build()
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		kit.title("Invites"),
		kit.hint("Here is a list of all active invite links to this channel. You can revoke any one."),
		ui.column {
			w_full = true,
			kit.table_head(COLUMNS),
			ui.each(ui.computed(function()
				local value = invites()
				return if value then value.rows else {}
			end), row, {
				key = function(entry: state.ServerInviteRow)
					return entry.id .. "|" .. entry.uses .. "|" .. entry.expires .. "|" .. entry.avatar
				end,
				w_full = true,
			}),
			ui.show(function()
				local value = invites()
				return value ~= nil and value.loaded and #value.rows == 0
			end, kit.empty("mail", "There are no active invites to this channel.")),
		},
	}
end

return {
	keywords = "channel invites invite links codes revoke copy",
	build = build,
}
