local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local kit = require("./kit")

local query = ui.state("bans_query", "")

local function bans()
	return state.server.bans:get()
end

local COLUMNS: { kit.Column } = {
	{ title = "User", grow = 1 },
	{ title = "Reason", grow = 1.4 },
}

local function row(entry: state.ServerBanRow)
	return kit.table_row {
		id = "ban_revoke_" .. entry.id,
		label = "Revoke the ban on " .. entry.name,
		columns = COLUMNS,
		on_click = function()
			actions.server_action("unban", { id = entry.id, name = entry.name })
		end,
		cells = {
			kit.person(entry.avatar, entry.name, nil, entry.username),
			kit.text(entry.reason, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_3, {
				truncate = true,
				min_w = 0,
			}),
		},
		actions = function(hovered: () -> boolean)
			return kit.row_actions(
				hovered,
				ui.row {
					w = theme.size.space_24,
					h = theme.size.space_24,
					rounded = theme.size.space_24,
					justify = "center",
					items = "center",
					bg = theme.color.bg_4,
					tooltip = "Revoke Ban",
					require("@components/icon") {
						name = "close",
						size = theme.size.icon_size_sm :: any,
						color = theme.color.red_2,
					},
				}
			)
		end,
	}
end

local function build()
	ui.effect(function()
		actions.server_action("bans_search", query:get())
	end)
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		kit.title("Server Ban List"),
		kit.hint("Bans by default are by account and IP. Click a ban to revoke it."),
		ui.row {
			w_full = true,
			kit.search_input(query, "Search Bans by User Id or Username", { id = "bans_search" }),
		},
		kit.text(function()
			local value = bans()
			local count = if value then #value.rows else 0
			return string.format("%d %s", count, if count == 1 then "ban" else "bans")
		end, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, { weight = theme.weight.medium }),
		ui.column {
			w_full = true,
			kit.table_head(COLUMNS),
			ui.each(ui.computed(function()
				local value = bans()
				return if value then value.rows else {}
			end), row, {
				key = function(entry: state.ServerBanRow)
					return entry.id .. "|" .. entry.name .. "|" .. entry.reason .. "|" .. entry.avatar
				end,
				w_full = true,
			}),
			ui.show(function()
				local value = bans()
				return value ~= nil and value.loaded and #value.rows == 0
			end, kit.empty("ban", "Nobody is banned from this server.")),
		},
		ui.show(function()
			local value = bans()
			return value ~= nil and value.has_more
		end, ui.row {
			w_full = true,
			justify = "center",
			kit.button {
				id = "bans_more",
				text = "Load more",
				on_click = function()
					actions.server_action("bans_more")
				end,
			},
		}),
	}
end

return {
	keywords = "bans banned unban revoke",
	build = build,
}
