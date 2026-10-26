local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local kit = require("./kit")

local query = ui.state("members_query", "")

local function n(token: string): number
	return theme.number(token)()
end

local function members()
	return state.server.members:get()
end

local function count_text(count: number): string
	local digits = tostring(math.floor(count))
	local grouped = string.reverse((string.gsub(string.reverse(digits), "(%d%d%d)", "%1,")))
	return (string.gsub(grouped, "^,", ""))
end

local COLUMNS: { kit.Column } = {
	{ title = "Name", grow = 1.6 },
	{ title = "Member since", grow = 1 },
	{ title = "Roles", grow = 1.4 },
	{ title = "", w = function()
		return n(theme.size.space_24)
	end },
}

local function open_menu(entry: state.ServerMemberRow)
	return function(event: any)
		actions.server_action("member_menu", { id = entry.id, x = event.x, y = event.y })
		return nil
	end
end

local function row(entry: state.ServerMemberRow)
	local chips = {}
	for _, chip in entry.roles do
		table.insert(chips, kit.chip(chip.name, chip.color))
	end
	return kit.table_row {
		id = "member_row_" .. entry.id,
		columns = COLUMNS,
		context_menu = open_menu(entry),
		cells = {
			kit.person(entry.avatar, entry.name, entry.color, entry.username),
			kit.text(entry.since or "", theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_3, {
				truncate = true,
				min_w = 0,
			}),
			ui.row { min_w = 0, flex_wrap = true, gap = theme.size.space_4, table.unpack(chips) },
			ui.show(function()
				local value = members()
				return value ~= nil and value.actions
			end, ui.button {
				id = "member_more_" .. entry.id,
				w = theme.size.space_24,
				h = theme.size.space_24,
				shrink = 0,
				p = 0,
				border = 0,
				focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
				rounded = theme.size.radius_sm,
				justify = "center",
				items = "center",
				cursor = "pointer",
				tooltip = "More",
				label = "More",
				bg = "#00000000",
				hover = { bg = theme.color.hover },
				on_press = function(event: any)
					actions.server_action("member_menu", { id = entry.id, x = event.x, y = event.y })
				end,
				icon { name = "more", size = theme.size.icon_size_sm :: any, color = theme.color.text_4 },
			}),
		},
	}
end

local function build()
	ui.effect(function()
		actions.server_action("members_search", query:get())
	end)
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		kit.title("Server Members"),
		ui.row {
			w_full = true,
			gap = theme.size.space_12,
			kit.search_input(query, "Search by username or id", {
				id = "members_search",
				on_key = function(event: any): boolean?
					if event.key == "Enter" then
						actions.server_action("members_remote", query:get())
						return true
					end
					return false
				end,
			}),
			kit.select {
				id = "members_sort",
				labels = state.server.member_sorts:peek(),
				w = n(theme.size.settings_column_width) * 0.32,
				current = function()
					local value = members()
					return (if value then value.sort or 0 else 0) + 1
				end,
				pick = function(index: number)
					actions.server_action("members_sort", index - 1)
				end,
			},
		},
		kit.text(function()
			local value = members()
			if not value then
				return ""
			end
			local count = value.count or value.total
			return string.format(
				"Showing %s of %s %s",
				count_text(value.shown or #value.rows),
				count_text(count),
				if count == 1 then "member" else "members"
			)
		end, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, { weight = theme.weight.medium }),
		ui.column {
			w_full = true,
			kit.table_head(COLUMNS),
			ui.each(ui.computed(function()
				local value = members()
				return if value then value.rows else {}
			end), row, {
				key = function(entry: state.ServerMemberRow)
					return entry.id .. "|" .. entry.name .. "|" .. entry.avatar .. "|" .. entry.color .. "|" .. (entry.since or "")
				end,
				w_full = true,
			}),
			ui.show(function()
				local value = members()
				return value ~= nil and #value.rows == 0
			end, kit.empty("members", "No members match your search.")),
		},
	}
end

return {
	keywords = "members people search kick ban timeout nickname roles member since",
	build = build,
}
