local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local avatar = require("@components/avatar")
local kit = require("./kit")

local expanded = ui.state("expanded", {} :: { [string]: boolean })

local TONES: { [string]: string } = {
	create = theme.color.green_2,
	update = theme.color.yellow_2,
	delete = theme.color.red_2,
}

local function n(token: string): number
	return theme.number(token)()
end

local function audit()
	return state.server.audit:get()
end

local function people_labels(): { string }
	local labels = { "All" }
	local value = audit()
	for _, person in (value and value.people) or {} do
		table.insert(labels, person.name)
	end
	return labels
end

local function action_labels(): { string }
	local labels = {}
	for index, label in state.server.audit_filters:get() do
		table.insert(labels, if index == 1 then "All" else label)
	end
	return labels
end

local function sentence(entry: state.ServerAuditRow)
	local props: { [string]: any } = {
		wrap = true,
		min_w = 0,
		text_size = theme.size.font_size_md,
		line_height = theme.size.line_height_label,
		color = theme.color.text_3,
	}
	local parts = entry.parts or { { text = entry.sentence, strong = false } }
	for _, part in parts do
		if part.strong then
			table.insert(props, ui.span { part.text, literal = true, weight = theme.weight.semibold, color = theme.color.text_2 })
		else
			table.insert(props, ui.span { part.text, literal = true })
		end
	end
	return ui.rich(props)
end

local function change(line: state.ServerAuditChange)
	local props: { [string]: any } = {
		wrap = true,
		text_size = theme.size.font_size_sm,
		line_height = theme.size.line_height_sm,
		color = theme.color.text_3,
	}
	local function add(text: string, extra: { [string]: any }?)
		local span: { [string]: any } = { text, literal = true }
		for key, value in extra or {} do
			span[key] = value
		end
		table.insert(props, ui.span(span))
	end
	add("Changed ")
	add(line.key, { weight = theme.weight.semibold })
	if line.old ~= "" then
		add(" from ")
		add(line.old, { color = theme.color.text_4 })
	end
	if line.new ~= "" then
		add(" to ")
		add(line.new, { color = theme.color.accent_1 })
	end
	return ui.rich(props)
end

local function details(entry: state.ServerAuditRow): boolean
	return #entry.changes > 0 or entry.reason ~= ""
end

local function entry_card(entry: state.ServerAuditRow)
	local lines = {}
	for _, line in entry.changes do
		table.insert(lines, change(line))
	end
	if entry.reason ~= "" then
		table.insert(
			lines,
			ui.rich {
				wrap = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				color = theme.color.text_3,
				ui.span { "Reason: ", literal = true, weight = theme.weight.semibold },
				ui.span { entry.reason, literal = true },
			}
		)
	end
	local function open(): boolean
		return expanded:get()[entry.id] == true
	end
	local inset = n(theme.size.space_16) + n(theme.size.icon_size_md) + n(theme.size.space_12) * 2 + n(theme.size.avatar_size_md)
	local side = n(theme.size.avatar_size_md)
	return ui.column {
		id = "audit_entry_" .. entry.id,
		w_full = true,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		ui.button {
			id = "audit_toggle_" .. entry.id,
			label = entry.sentence,
			w_full = true,
			gap = theme.size.space_12,
			px = theme.size.space_16,
			py = theme.size.space_12,
			items = "center",
			border = 0,
			focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
			rounded = theme.size.radius_sm,
			bg = "#00000000",
			cursor = if details(entry) then "pointer" else nil,
			on_click = function()
				if details(entry) then
					local next = table.clone(expanded:get())
					next[entry.id] = if open() then nil else true
					expanded:set(next)
				end
			end,
			icon {
				name = entry.icon or "audit_log",
				size = theme.size.icon_size_md :: any,
				color = TONES[entry.tone or "update"] or theme.color.text_4,
			},
			ui.div {
				w = side,
				h = side,
				shrink = 0,
				rounded = side,
				bg = theme.color.bg_2,
				if entry.avatar ~= "" then avatar { src = entry.avatar, size = side } else nil,
			},
			ui.column {
				grow = 1,
				min_w = 0,
				sentence(entry),
				kit.text(entry.when, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_4),
			},
			if details(entry)
				then ui.show(
					open,
					icon { name = "chevron_up", size = theme.size.icon_size_md :: any, color = theme.color.text_4 },
					icon { name = "chevron_down", size = theme.size.icon_size_md :: any, color = theme.color.text_4 }
				)
				else nil,
		},
		ui.show(open, ui.column {
			w_full = true,
			gap = theme.size.space_6,
			pt = theme.size.space_12,
			pr = theme.size.space_16,
			pb = theme.size.space_16,
			pl = inset,
			border_sides = { 1, 0, 0, 0 },
			border_color = theme.color.border,
			table.unpack(lines),
		}),
	}
end

local function build()
	expanded:set({})
	return ui.column {
		w_full = true,
		gap = theme.size.space_12,
		kit.title("Audit Log"),
		ui.row {
			w_full = true,
			gap = theme.size.space_12,
			ui.presence(function()
				return table.concat(people_labels(), "\n")
			end, function()
				local value = audit()
				local people = (value and value.people) or {}
				return kit.select {
					id = "audit_user",
					prefix = "Filter by User: ",
					grow = 1,
					labels = people_labels(),
					current = function()
						local found = audit()
						return (if found then found.person or 0 else 0) + 1
					end,
					pick = function(index: number)
						local person = people[index - 1]
						actions.server_action("audit_user", if person then person.id else "")
					end,
				}
			end),
			kit.select {
				id = "audit_action",
				prefix = "Filter by Action: ",
				grow = 1,
				labels = action_labels(),
				current = function()
					local value = audit()
					return (if value then value.filter else 0) + 1
				end,
				pick = function(index: number)
					actions.server_action("audit_filter", index - 1)
				end,
			},
		},
		ui.each(ui.computed(function()
			local value = audit()
			return if value then value.rows else {}
		end), entry_card, {
			key = function(entry: state.ServerAuditRow)
				return entry.id .. "|" .. entry.avatar .. "|" .. entry.sentence
			end,
			w_full = true,
			gap = theme.size.space_12,
		}),
		ui.show(function()
			local value = audit()
			return value ~= nil and value.loaded and #value.rows == 0
		end, kit.empty("audit_log", "No entries match these filters.")),
		ui.show(function()
			local value = audit()
			return value ~= nil and value.has_more
		end, ui.row {
			w_full = true,
			justify = "center",
			pt = theme.size.space_4,
			kit.button {
				id = "audit_more",
				text = "Load more",
				on_click = function()
					actions.server_action("audit_more")
				end,
			},
		}),
	}
end

return {
	keywords = "audit log history actions moderation filter",
	build = build,
}
