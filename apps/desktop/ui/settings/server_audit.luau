local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local widgets = require("@settings/widgets")
local kit = require("@settings/server_kit")

local AVATAR = 32
local SPINE = 2

local function audit()
	return state.server.audit:get()
end

local function change(line: state.ServerAuditChange)
	return ui.row {
		w_full = true,
		gap = theme.size.space_8,
		widgets.label(line.key, theme.size.font_sm, theme.color.header_secondary, {
			weight = theme.weight.semibold,
			literal = true,
			shrink = 0,
		}),
		if line.old ~= ""
			then widgets.label(line.old, theme.size.font_sm, theme.color.danger, {
				literal = true,
				truncate = true,
				min_w = 0,
			})
			else nil,
		if line.new ~= ""
			then widgets.label(line.new, theme.size.font_sm, theme.color.success, {
				literal = true,
				truncate = true,
				min_w = 0,
			})
			else nil,
	}
end

local function row(entry: state.ServerAuditRow)
	local lines = {}
	for _, line in entry.changes do
		table.insert(lines, change(line))
	end
	return ui.row {
		w_full = true,
		gap = theme.size.space_12,
		p = theme.size.space_12,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.divider,
		avatar { src = entry.avatar, size = AVATAR },
		ui.column {
			grow = 1,
			min_w = 0,
			gap = theme.size.space_4,
			ui.row {
				w_full = true,
				items = "center",
				justify = "between",
				gap = theme.size.space_8,
				widgets.label(entry.sentence, theme.size.font_md, theme.color.text_normal, {
					literal = true,
					min_w = 0,
					wrap = true,
				}),
				widgets.label(entry.when, theme.size.font_tiny, theme.color.text_muted, { literal = true, shrink = 0 }),
			},
			if #lines > 0
				then ui.column {
					w_full = true,
					gap = theme.size.space_2,
					pl = theme.size.space_8,
					border_sides = { 0, 0, 0, SPINE },
					border_color = theme.color.divider,
					table.unpack(lines),
				}
				else nil,
			if entry.reason ~= ""
				then widgets.label("Reason: " .. entry.reason, theme.size.font_sm, theme.color.text_muted, {
					literal = true,
					italic = true,
					wrap = true,
				})
				else nil,
		},
	}
end

return function()
	return ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		kit.title("Audit Log"),
		ui.row {
			shrink = 0,
			items = "center",
			gap = theme.size.space_12,
			pb = theme.size.space_16,
			kit.select {
				labels = state.server.audit_filters:peek(),
				current = function()
					local value = audit()
					return (if value then value.filter else 0) + 1
				end,
				pick = function(index: number)
					actions.server_action("audit_filter", index - 1)
				end,
			},
		},
		kit.rows(audit, row, kit.empty("audit_log", "No entries match this filter.")),
		ui.show(function()
			local value = audit()
			return value ~= nil and value.has_more
		end, ui.row {
			w_full = true,
			justify = "center",
			pt = theme.size.space_16,
			pb = theme.size.space_16,
			kit.button {
				text = "Load more",
				tone = "outline",
				on_click = function()
					actions.server_action("audit_more")
				end,
			},
		}),
	}
end
