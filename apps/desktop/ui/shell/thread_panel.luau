local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local composer = require("./composer")

local SLOT = "thread"
local GLYPH = 24
local MEMBERS_GLYPH = 16
local NOTICE_BUTTON = 24

local function panel(): state.ThreadPanel
	return state.threads.panel:get()
end

local function header()
	return ui.row {
		w_full = true,
		h = theme.size.top_bar_height,
		shrink = 0,
		items = "center",
		gap = theme.size.space_8,
		px = theme.size.space_16,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.background_tertiary,
		icon { name = "thread", size = GLYPH, color = theme.color.text_muted },
		ui.text {
			function()
				return panel().name
			end,
			literal = true,
			grow = 1,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_md,
			weight = theme.weight.semibold,
			color = theme.color.header_primary,
		},
		ui.row {
			shrink = 0,
			items = "center",
			gap = theme.size.space_4,
			icon { name = "members", size = MEMBERS_GLYPH, color = theme.color.text_muted },
			ui.text {
				function()
					return tostring(panel().members)
				end,
				text_size = theme.size.font_sm,
				color = theme.color.text_muted,
			},
		},
		ui.show(function()
			return panel().can_manage
		end, icon_button {
			name = function()
				return if panel().archived then "unarchive" else "archive"
			end,
			tooltip = ui.text { function()
				return if panel().archived then "Unarchive Thread" else "Archive Thread"
			end },
			on_click = function()
				local current = state.threads.panel:peek()
				actions.archive_thread(current.id, not current.archived)
			end,
		}),
		icon_button {
			id = "thread_membership",
			name = function()
				return if panel().joined then "log_out" else "join"
			end,
			tooltip = ui.text { function()
				return if panel().joined then "Leave Thread" else "Join Thread"
			end },
			on_click = function()
				local current = state.threads.panel:peek()
				if current.joined then
					actions.leave_thread(current.id)
				else
					actions.join_thread(current.id)
				end
			end,
		},
		icon_button {
			id = "thread_close",
			name = "close",
			tooltip = "Close Thread",
			on_click = function()
				actions.close_thread()
			end,
		},
	}
end

local function notice_button(id: string, label: string, on_click: () -> ())
	return ui.button {
		label,
		id = id,
		h = NOTICE_BUTTON,
		px = theme.size.space_8,
		py = 0,
		shrink = 0,
		rounded = theme.size.radius_sm,
		border = 0,
		focus = { border = 0 },
		text_size = theme.size.font_sm,
		weight = theme.weight.medium,
		color = theme.color.text_on_brand,
		bg = theme.color.brand,
		hover = { bg = theme.color.brand_hover },
		on_click = on_click,
	}
end

local function notice()
	return ui.show(function()
		local current = panel()
		return current.archived or not current.joined
	end, ui.row {
		w_full = true,
		shrink = 0,
		mb = theme.size.space_16,
		px = theme.size.space_16,
		ui.row {
			w_full = true,
			items = "center",
			justify = "between",
			gap = theme.size.space_8,
			px = theme.size.space_12,
			py = theme.size.space_8,
			rounded = theme.size.radius_md,
			bg = theme.color.background_secondary,
			ui.text {
				function()
					local current = panel()
					if current.archived then
						return "This thread is archived."
					end
					return "Join " .. current.name .. " to take part in the conversation."
				end,
				literal = true,
				grow = 1,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_sm,
				color = theme.color.text_muted,
			},
			ui.show(function()
				return not panel().archived
			end, notice_button("thread_join_notice", "Join Thread", function()
				actions.join_thread(state.threads.panel:peek().id)
			end), ui.show(function()
				return panel().can_manage
			end, notice_button("thread_unarchive_notice", "Unarchive Thread", function()
				actions.archive_thread(state.threads.panel:peek().id, false)
			end))),
		},
	})
end

return function()
	return ui.column {
		w = theme.size.thread_panel_width,
		h_full = true,
		shrink = 0,
		min_h = 0,
		header(),
		ui.column {
			grow = 1,
			min_h = 0,
			w_full = true,
			on_resize = function(_, element)
				actions.report_slot(SLOT, element)
			end,
		},
		notice(),
		ui.show(function()
			return not panel().archived
		end, composer {
			area = "thread_composer",
			shared = state.thread_composer,
			text_key = "thread_composer_text",
			measure_key = "thread_composer_measured",
		}),
	}
end
