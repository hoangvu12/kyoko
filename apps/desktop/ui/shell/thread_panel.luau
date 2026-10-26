local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local avatar = require("@components/avatar")
local composer = require("./composer")

local SLOT = "thread"

local icon_md = theme.number(theme.size.icon_size_md)
local button = theme.number(theme.size.button_size)
local avatar_md = theme.number(theme.size.avatar_size_md)
local avatar_lg = theme.number(theme.size.avatar_size_lg)
local switch_width = theme.number(theme.size.switch_width)
local switch_height = theme.number(theme.size.switch_height)
local switch_thumb = theme.number(theme.size.switch_thumb_size)
local border = theme.number(theme.size.border_thickness)

local function panel(): state.ThreadPanel
	return state.threads.panel:get()
end

local function tool(props: { [string]: any })
	props.hit_size = button()
	props.icon_size = icon_md()
	return icon_button(props)
end

local function words(value: any, size: string, extra: { [string]: any }?)
	local node = {
		value,
		literal = true,
		text_size = (theme.size :: any)["font_size_" .. size],
		line_height = if size == "md" then theme.size.line_height_label else (theme.size :: any)["line_height_" .. size],
	}
	for key, item in extra or {} do
		node[key] = item
	end
	return ui.text(node)
end

local function header(title: () -> string, tools: { any })
	local row = {
		w_full = true,
		h = theme.size.panel_header_height,
		shrink = 0,
		items = "center",
		gap = theme.size.space_8,
		pl = theme.size.space_16,
		pr = theme.size.space_8,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.border,
		icon { name = "thread", size = icon_md(), color = theme.color.text_4 },
		words(title, "md", {
			grow = 1,
			min_w = 0,
			truncate = true,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		}),
	}
	for _, item in tools do
		table.insert(row, item)
	end
	return ui.row(row)
end

local function close_tool()
	return tool {
		id = "thread_close",
		name = "close",
		tooltip = "Close",
		on_click = function()
			actions.close_thread()
		end,
	}
end

local function thread_header()
	return header(function()
		return panel().name
	end, {
		ui.show(function()
			return panel().can_manage
		end, tool {
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
		tool {
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
		close_tool(),
	})
end

local function tag(label: string)
	return ui.row {
		h = theme.size.space_20,
		shrink = 0,
		items = "center",
		px = theme.size.space_6,
		rounded = theme.size.space_32,
		bg = theme.color.bg_2,
		border = theme.size.border_thickness,
		border_color = theme.color.border_light,
		words(label, "xs", { weight = theme.weight.medium, color = theme.color.text_3 }),
	}
end

local function post_tags()
	return ui.show(function()
		return panel().post and #panel().tags > 0
	end, ui.row {
		w_full = true,
		shrink = 0,
		flex_wrap = true,
		gap = theme.size.space_6,
		pt = theme.size.space_12,
		px = theme.size.space_16,
		ui.each(ui.computed(function()
			return panel().tags
		end), function(label: string)
			return tag(label)
		end, { flex_wrap = true, gap = theme.size.space_6 }),
	})
end

local function notice_button(id: string, label: string, on_click: () -> ())
	return ui.button {
		label,
		id = id,
		h = theme.size.button_height_sm,
		px = theme.size.space_16,
		py = 0,
		shrink = 0,
		rounded = theme.size.radius_sm,
		border = theme.size.border_thickness,
		border_color = theme.color.button_border,
		focus = { border = border() },
		text_size = theme.size.font_size_sm,
		weight = theme.weight.medium,
		color = theme.color.text_0,
		bg = theme.color.accent_3,
		hover = { bg = theme.color.accent_4 },
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
		mb = theme.size.space_8,
		px = theme.size.space_8,
		ui.row {
			w_full = true,
			items = "center",
			justify = "between",
			gap = theme.size.space_8,
			px = theme.size.space_12,
			py = theme.size.space_8,
			rounded = theme.size.radius_md,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border,
			words(function()
				local current = panel()
				if current.archived then
					return "This thread is archived."
				end
				return "Join " .. current.name .. " to take part in the conversation."
			end, "sm", { grow = 1, min_w = 0, truncate = true, color = theme.color.text_4 }),
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

local function viewing()
	return ui.column {
		w_full = true,
		h_full = true,
		min_h = 0,
		thread_header(),
		post_tags(),
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

local function caption(value: string)
	return words(string.upper(value), "xs", {
		mb = theme.size.space_8,
		weight = theme.weight.bold,
		color = theme.color.text_4,
	})
end

local function starter()
	return ui.show(function()
		return panel().starter_text ~= "" or panel().starter_author ~= ""
	end, ui.column {
		w_full = true,
		px = theme.size.space_12,
		py = theme.size.space_8,
		rounded = theme.size.radius_md,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		words("STARTER MESSAGE", "xs", { weight = theme.weight.bold, color = theme.color.text_4 }),
		ui.row {
			w_full = true,
			mt = theme.size.space_8,
			gap = theme.size.space_16,
			items = "start",
			ui.presence(function()
				return panel().starter_avatar
			end, function(src: string)
				return avatar { src = src, size = avatar_md() }
			end),
			ui.column {
				grow = 1,
				min_w = 0,
				ui.row {
					items = "end",
					gap = theme.size.space_4,
					words(function()
						return panel().starter_author
					end, "md", { weight = theme.weight.medium, color = theme.color.text_2 }),
					words(function()
						return panel().starter_time
					end, "xs", { weight = theme.weight.medium, color = theme.color.text_5 }),
				},
				words(function()
					return panel().starter_text
				end, "md", { wrap = true, lines = 3, color = theme.color.text_3 }),
			},
		},
	})
end

local function private_switch(on: any)
	return ui.button {
		id = "thread_private",
		relative = true,
		w = theme.size.switch_width,
		h = theme.size.switch_height,
		shrink = 0,
		p = 0,
		rounded = theme.size.switch_height,
		border = theme.size.border_thickness,
		border_color = function()
			return if on:get() then theme.color.button_border else theme.color.border
		end,
		focus = { border = border() },
		role = "switch",
		label = "Private Thread",
		checked = function()
			return on:get()
		end,
		cursor = "pointer",
		bg = function()
			return if on:get() then theme.color.accent_3 else theme.color.bg_3
		end,
		on_click = function()
			on:set(not on:peek())
		end,
		ui.div {
			absolute = true,
			top = function()
				return (switch_height() - border() * 2 - switch_thumb()) / 2
			end,
			left = function()
				local inset = (switch_height() - border() * 2 - switch_thumb()) / 2
				return if on:get() then switch_width() - border() * 2 - switch_thumb() - inset else inset
			end,
			w = theme.size.switch_thumb_size,
			h = theme.size.switch_thumb_size,
			rounded = theme.size.switch_thumb_size,
			bg = function()
				return if on:get() then theme.color.bg_4 else theme.color.text_3
			end,
		},
	}
end

local creating = ui.component(function()
	local name = ui.use_state(state.threads.panel:peek().draft_name)
	local message = ui.use_state("")
	local private = ui.use_state(false)
	local function submit()
		if string.match(name:peek(), "%S") or string.match(message:peek(), "%S") then
			actions.create_thread(name:peek(), message:peek(), private:peek())
		end
	end
	return ui.column {
		w_full = true,
		h_full = true,
		min_h = 0,
		header(function()
			return "New Thread"
		end, { close_tool() }),
		ui.column {
			grow = 1,
			min_h = 0,
			w_full = true,
			overflow = "hidden",
			px = theme.size.space_16,
			py = theme.size.space_24,
			gap = theme.size.space_20,
			ui.row {
				w = theme.size.avatar_size_lg,
				h = theme.size.avatar_size_lg,
				shrink = 0,
				justify = "center",
				items = "center",
				rounded = theme.size.avatar_size_lg,
				bg = theme.color.bg_3,
				icon { name = "thread", size = avatar_lg() / 2, color = theme.color.text_3 },
			},
			ui.column {
				w_full = true,
				caption("Thread Name"),
				ui.input {
					id = "thread_name",
					w_full = true,
					h = theme.size.input_height,
					px = theme.size.space_12,
					rounded = theme.size.radius_sm,
					bg = theme.color.bg_3,
					border = theme.size.border_thickness,
					border_color = theme.color.border_light,
					focus = { border_color = theme.color.accent_2 },
					text_size = theme.size.font_size_md,
					line_height = theme.size.line_height_label,
					color = theme.color.text_3,
					placeholder = "New Thread",
					placeholder_color = theme.color.text_5,
					value = name,
					on_key = function(event: any): boolean?
						if event.key == "Enter" then
							submit()
							return true
						end
						return nil
					end,
				},
			},
			starter(),
			ui.show(function()
				return panel().can_private
			end, ui.row {
				w_full = true,
				items = "center",
				gap = theme.size.space_16,
				ui.column {
					grow = 1,
					min_w = 0,
					words("Private Thread", "md", { weight = theme.weight.semibold, color = theme.color.text_2 }),
					words("Only people you invite and moderators can see this thread.", "sm", {
						mt = theme.size.space_2,
						wrap = true,
						color = theme.color.text_4,
					}),
				},
				private_switch(private),
			}),
		},
		ui.row {
			h = theme.size.chatbar_height,
			shrink = 0,
			mx = theme.size.space_8,
			mb = theme.size.space_8,
			items = "center",
			px = theme.size.space_16,
			rounded = theme.size.radius_md,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border_light,
			ui.input {
				id = "thread_first_message",
				grow = 1,
				min_w = 0,
				h_full = true,
				p = 0,
				bg = "#00000000",
				border = 0,
				focus = { border = 0 },
				text_size = theme.size.font_size_md,
				line_height = theme.size.line_height_md,
				color = theme.color.text_3,
				placeholder = "Enter a message to start the conversation!",
				placeholder_color = theme.color.text_5,
				value = message,
				on_key = function(event: any): boolean?
					if event.key == "Enter" then
						submit()
						return true
					end
					return nil
				end,
			},
		},
	}
end)

return function()
	return ui.column {
		w = function()
			return if panel().post then theme.size.forum_post_panel_width else theme.size.thread_panel_width
		end,
		h_full = true,
		shrink = 0,
		min_h = 0,
		ui.show(function()
			return panel().creating
		end, creating(), viewing()),
	}
end
