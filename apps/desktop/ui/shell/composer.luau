local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local plugin_slot = require("@components/plugin_slot")
local command_picker = require("./command_picker")

local LEADING = 56
local LINE = 22
local TEXT_SIZE = 16
local MAX_LINES = 10
local EMOJI = 20
local HINT = "↑↓ to navigate, ↵/tab to select, esc to dismiss"
local COUNTER_FROM = 200
local DOT = 6
local DOT_DIM = 0.4
local DOT_STEP = 300
local CARD = 216
local CARD_PADDING = 8
local THUMBNAIL = 160
local TOOL = 24
local TOOL_ICON = 16
local BADGE = 40
local SPOILER_BLUR = 32
local CLOSE = 24
local CLOSE_ICON = 12
local ICON = 20
local BUTTON = 32

local function by_key(item: { key: string })
	return item.key
end

local function by_id(item: { id: string })
	return item.id
end

local function length(text: string): number
	return utf8.len(text) or #text
end

local function is_locked(item: state.ComposerSuggestion): boolean
	return item.lock ~= nil and item.lock ~= ""
end

local function suggestion_visual(item: state.ComposerSuggestion)
	if item.kind == "glyph" and item.svg and item.svg ~= "" then
		return ui.svg { code = item.svg, w = EMOJI, h = EMOJI, shrink = 0 }
	elseif item.kind == "glyph" then
		return ui.text { item.glyph, literal = true, text_size = EMOJI, w = EMOJI, align = "center", shrink = 0 }
	elseif item.kind == "emoji" then
		return ui.row {
			w = EMOJI,
			h = EMOJI,
			shrink = 0,
			saturate = if is_locked(item) then 0 else 1,
			opacity = if is_locked(item) then theme.number(theme.size.locked_opacity)() else 1,
			ui.show(item.image ~= "", ui.image { src = item.image, w = EMOJI, h = EMOJI, fit = "contain" }),
		}
	elseif item.kind == "icon" then
		return icon { name = item.icon, size = EMOJI, color = theme.color.text_4 }
	elseif item.kind == "avatar" then
		return ui.row {
			w = 24,
			h = 24,
			shrink = 0,
			rounded = 12,
			overflow = "hidden",
			bg = theme.color.background_secondary,
			ui.show(item.image ~= "", ui.image { src = item.image, w = 24, h = 24, fit = "cover" }),
		}
	end
	return ui.row {
		w = 24,
		h = 24,
		shrink = 0,
		ui.show(item.image ~= "", ui.image { src = item.image, w = 24, h = 24, fit = "contain" }),
	}
end

local function suggestion_row(item: state.ComposerSuggestion, index: number, text: any, shared: any, area: string)
	local locked = is_locked(item)
	local function selected(): boolean
		return shared.selected:get() == index
	end
	return ui.row {
		id = area .. "_suggestion_" .. index,
		w_full = true,
		h = theme.size.autocomplete_row_height,
		px = theme.size.space_8,
		gap = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		cursor = if locked then "default" else "pointer",
		bg = function()
			return if selected() then theme.color.hover else "#00000000"
		end,
		hover = { bg = theme.color.hover },
		on_click = function()
			actions.composer_choose(index, text:peek(), area)
		end,
		suggestion_visual(item),
		ui.text {
			item.label,
			literal = true,
			min_w = 0,
			shrink = 0,
			truncate = true,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = function()
				return if selected() then theme.color.text_2 else theme.color.text_3
			end,
		},
		ui.show(
			locked,
			ui.row {
				shrink = 0,
				tooltip = item.lock,
				icon { name = "lock", size = theme.size.icon_size_xs :: any, color = theme.color.text_4 },
			}
		),
		ui.show(item.detail ~= "", ui.text {
			item.detail,
			literal = true,
			grow = 1,
			min_w = 0,
			align = "right",
			truncate = true,
			text_size = theme.size.font_size_xs,
			line_height = theme.size.line_height_xs,
			color = theme.color.text_5,
		}),
	}
end

local function popup_line(content: { any }, justify: string)
	local props: { [any]: any } = {
		w_full = true,
		h = theme.size.space_24,
		shrink = 0,
		px = theme.size.space_8,
		items = "center",
		justify = justify,
		text_size = theme.size.font_size_xs,
		line_height = theme.size.line_height_xs,
	}
	for _, child in content do
		table.insert(props, child)
	end
	return ui.row(props)
end

local function popup(text: any, shared: any, area: string, suggesting: () -> boolean)
	return ui.show(suggesting, ui.column {
		id = area .. "_suggestions",
		mx = theme.size.space_8,
		p = theme.size.space_8,
		gap = theme.size.space_2,
		shrink = 0,
		rounded_tl = theme.number(theme.size.radius_md),
		rounded_tr = theme.number(theme.size.radius_md),
		border = 1,
		border_sides = { 1, 1, 0, 1 },
		border_color = theme.color.border,
		bg = theme.color.bg_3,
		color = theme.color.text_3,
		popup_line({
			ui.text {
				function()
					return string.upper(shared.heading:get().title or "")
				end,
				literal = true,
				shrink = 0,
				weight = theme.weight.semibold,
				color = theme.color.text_4,
			},
			ui.text {
				function()
					return shared.heading:get().query or ""
				end,
				literal = true,
				ml = theme.size.space_4,
				min_w = 0,
				truncate = true,
				weight = theme.weight.semibold,
				color = theme.color.text_2,
			},
		}, "start"),
		ui.each(shared.suggestions, function(item: state.ComposerSuggestion, index: number)
			return suggestion_row(item, index, text, shared, area)
		end, { key = by_key, w_full = true, gap = theme.size.space_2 }),
		popup_line({
			ui.text {
				HINT,
				literal = true,
				color = theme.color.text_5,
			},
		}, "end"),
	})
end

local function bar_button(label: string, on_click: () -> (), danger: boolean)
	return ui.button {
		label,
		h = 24,
		px = theme.size.space_8,
		py = 0,
		rounded = theme.size.radius_sm,
		border = 0,
		focus = { border = 0 },
		text_size = theme.size.font_size_sm,
		weight = theme.weight.medium,
		color = theme.color.text_0,
		bg = if danger then theme.color.red_3 else theme.color.accent_3,
		hover = { bg = if danger then theme.color.red_4 else theme.color.accent_4 },
		on_click = on_click,
	}
end

local function close_disc(id: string, tooltip: string, on_click: () -> ())
	return ui.button {
		id = id,
		w = CLOSE,
		h = CLOSE,
		shrink = 0,
		p = 0,
		rounded = CLOSE,
		border = 0,
		focus = { border = 0 },
		items = "center",
		justify = "center",
		cursor = "pointer",
		tooltip = tooltip,
		bg = theme.color.bg_2,
		hover = { bg = theme.color.bg_1 },
		on_click = on_click,
		icon { name = "close", size = CLOSE_ICON, color = theme.color.text_3 },
	}
end

local typing_dots = ui.component(function()
	local phase = ui.use_state(0)
	local function animated(): boolean
		local values = state.settings.values:get() or {}
		return values["chat.typing_animation"] ~= false
			and values["accessibility.reduced_motion"] ~= true
			and not ui.prefers.reduced_motion:get()
	end
	ui.every(DOT_STEP, function()
		if animated() then
			phase:set((phase:get() + 1) % 3)
		elseif phase:get() ~= 0 then
			phase:set(0)
		end
	end)
	local dots = {}
	for index = 0, 2 do
		table.insert(
			dots,
			ui.div {
				id = "typing_dot_" .. index,
				w = DOT,
				h = DOT,
				rounded = DOT,
				bg = theme.color.text_3,
				opacity = function()
					if not animated() then
						return 1
					end
					return if phase:get() == index then 1 else DOT_DIM
				end,
			}
		)
	end
	return ui.row { shrink = 0, gap = theme.size.space_2, items = "center", table.unpack(dots) }
end)

local function upload_tool(id: string, name: string, tooltip: string, danger: boolean, on_click: () -> ())
	return ui.button {
		id = id,
		w = TOOL,
		h = TOOL,
		p = 0,
		shrink = 0,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_xs,
		items = "center",
		justify = "center",
		cursor = "pointer",
		tooltip = tooltip,
		bg = "#00000000",
		hover = { bg = theme.color.hover },
		on_click = on_click,
		icon { name = name, size = TOOL_ICON, color = if danger then theme.color.red_2 else theme.color.text_4 },
	}
end

local function upload_thumbnail(file: state.ComposerFile)
	return ui.row {
		relative = true,
		w_full = true,
		h = THUMBNAIL,
		shrink = 0,
		items = "center",
		justify = "center",
		overflow = "hidden",
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		ui.show(
			file.image ~= "",
			ui.image {
				src = file.image,
				absolute = true,
				left = 0,
				top = 0,
				right = 0,
				bottom = 0,
				fit = "cover",
				blur = if file.spoiler then SPOILER_BLUR else 0,
			},
			ui.show(file.kind ~= "video", icon { name = "file", size = BADGE, color = theme.color.text_4 })
		),
		ui.show(
			file.kind == "video",
			ui.row {
				absolute = true,
				left = (CARD - 2 * CARD_PADDING - BADGE) / 2,
				top = (THUMBNAIL - BADGE) / 2,
				w = BADGE,
				h = BADGE,
				rounded = BADGE,
				items = "center",
				justify = "center",
				bg = theme.color.overlay_button_background,
				icon { name = "play", size = 20, color = theme.color.overlay_text },
			}
		),
		ui.show(
			file.spoiler,
			ui.row {
				absolute = true,
				left = 0,
				top = 0,
				right = 0,
				bottom = 0,
				items = "center",
				justify = "center",
				ui.text {
					"SPOILER",
					h = 24,
					px = theme.size.space_12,
					rounded = 24,
					bg = theme.color.overlay_button_background,
					color = theme.color.overlay_text,
					text_size = theme.size.font_size_xs,
					line_height = 24,
					weight = theme.weight.bold,
				},
			}
		),
	}
end

local upload_card = ui.component(function(props: { file: state.ComposerFile, area: string })
	local hovered = ui.use_state(false)
	local file = props.file
	local area = props.area
	return ui.column {
		id = "upload_card_" .. file.id,
		relative = true,
		w = CARD,
		h = CARD,
		shrink = 0,
		p = CARD_PADDING,
		gap = theme.size.space_8,
		rounded = theme.size.radius_md,
		bg = theme.color.bg_4,
		border = 1,
		border_color = theme.color.border,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		upload_thumbnail(file),
		ui.text {
			file.name,
			literal = true,
			w_full = true,
			truncate = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			color = theme.color.text_3,
		},
		ui.show(function()
			return hovered:get()
		end, ui.row {
			absolute = true,
			top = 4,
			right = 4,
			w = 3 * TOOL + 6,
			p = theme.size.space_2,
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = 1,
			border_color = theme.color.border,
			upload_tool("upload_spoiler_" .. file.id, "eye", "Spoiler Attachment", false, function()
				actions.composer_spoiler(file.id, area)
			end),
			upload_tool("upload_edit_" .. file.id, "edit", "Modify Attachment", false, function()
				actions.composer_edit_file(file.id, area)
			end),
			upload_tool("upload_remove_" .. file.id, "trash", "Remove Attachment", true, function()
				actions.composer_remove_file(file.id, area)
			end),
		}),
	}
end)

export type Scope = {
	area: string,
	shared: any,
	text_key: string,
	measure_key: string,
}

local MAIN: Scope = {
	area = "composer",
	shared = state.composer,
	text_key = "composer_text",
	measure_key = "composer_measured",
}

return function(scope: any?)
	local current: Scope = if scope and scope.shared then scope else MAIN
	local room: (() -> number)? = scope and scope.room
	local area = current.area
	local shared = current.shared
	local picking = false
	local field_box: any = nil
	local chatbar = theme.number(theme.size.chatbar_height)
	local radius = theme.number(theme.size.radius_md)

	local function notice_kind(): string
		return shared.notice:get().kind
	end

	local function has_reply(): boolean
		return shared.reply:get().active == true
	end

	local function has_edit(): boolean
		return shared.editing:get() == true
	end

	local function has_files(): boolean
		return #shared.files:get() > 0
	end

	local function commanding(): boolean
		return shared.command:get().active == true
	end

	local function picker_open(): boolean
		return shared.picker:get().open == true
	end

	local function optional_open(): boolean
		return commanding() and shared.command:get().optional_open == true
	end

	local function choices_open(): boolean
		return commanding() and (shared.command:get().choices or {}).open == true
	end

	local function dropbox_open(): boolean
		return commanding() and shared.command:get().wants_file == true
	end

	local function joined(): boolean
		return notice_kind() ~= "" or has_reply() or has_edit() or has_files() or commanding()
	end

	local function suggesting(): boolean
		return #shared.suggestions:get() > 0
	end

	local function bar(content: { any }, background: any?)
		local props: { [any]: any } = {
			w_full = true,
			h = theme.size.composer_bar_height,
			shrink = 0,
			pl = theme.size.space_16,
			pr = theme.size.space_8,
			gap = theme.size.space_8,
			items = "center",
			bg = background,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			color = theme.color.text_4,
		}
		for _, child in content do
			table.insert(props, child)
		end
		return ui.row(props)
	end

	local function notice()
		local current_notice = shared.notice
		return ui.show(function()
			return notice_kind() ~= ""
		end, bar({
			ui.text {
				function()
					return current_notice:get().text or ""
				end,
				literal = true,
				grow = 1,
				min_w = 0,
				truncate = true,
				weight = theme.weight.medium,
				color = function()
					return if notice_kind() == "lock" then theme.color.text_3 else theme.color.red_2
				end,
			},
			ui.show(function()
				return notice_kind() == "lock" and current_notice:get().confirmable == true
			end, bar_button("I understand", function()
				actions.composer_confirm(area)
			end, false)),
			ui.show(function()
				return notice_kind() == "failed"
			end, ui.row {
				gap = theme.size.space_8,
				bar_button("Retry", function()
					actions.composer_retry(current_notice:peek().nonce or "", area)
				end, false),
				bar_button("Delete", function()
					actions.composer_discard(current_notice:peek().nonce or "", area)
				end, true),
			}),
			ui.show(function()
				return notice_kind() == "refused"
			end, close_disc(area .. "_notice_close", "Dismiss", function()
				actions.composer_dismiss(area)
			end)),
		}, function()
			return if notice_kind() == "lock"
				then theme.color.background_feedback_warning
				else theme.color.background_feedback_critical
		end))
	end

	local function reply_bar()
		local reply = shared.reply
		local function others(): boolean
			return reply:get().own ~= true
		end
		return ui.show(has_reply, bar({
			ui.row {
				grow = 1,
				min_w = 0,
				items = "center",
				ui.text { "Replying to ", literal = true, shrink = 0 },
				ui.text {
					function()
						return reply:get().name or ""
					end,
					literal = true,
					min_w = 0,
					truncate = true,
					weight = theme.weight.semibold,
					color = theme.color.text_2,
				},
			},
			ui.show(others, ui.button {
				id = area .. "_reply_ping",
				h = 24,
				px = theme.size.space_4,
				py = 0,
				shrink = 0,
				bg = "#00000000",
				border = 0,
				focus = { border = 0 },
				cursor = "pointer",
				tooltip = "Mention the author",
				text_size = theme.size.font_size_sm,
				weight = theme.weight.bold,
				color = function()
					return if reply:get().ping then theme.color.link else theme.color.text_4
				end,
				on_click = function()
					actions.composer_toggle_ping(area)
				end,
				ui.text(function()
					return if reply:get().ping then "@ ON" else "@ OFF"
				end),
			}),
			ui.show(others, ui.div { w = 1, h = 16, shrink = 0, bg = theme.color.border }),
			close_disc(area .. "_reply_close", "Cancel reply", function()
				actions.composer_cancel_reply(area)
			end),
		}))
	end

	local function edit_bar()
		return ui.show(has_edit, bar({
			ui.text {
				"Editing message · escape to cancel · enter to save",
				literal = true,
				grow = 1,
				min_w = 0,
				truncate = true,
			},
			close_disc(area .. "_edit_close", "Cancel edit", function()
				actions.composer_cancel_edit(area)
			end),
		}))
	end

	local function tray()
		return ui.show(has_files, ui.scroll_x {
			id = area .. "_upload_tray",
			w_full = true,
			shrink = 0,
			ui.row {
				p = theme.size.space_16,
				ui.each(shared.files, function(file: state.ComposerFile)
					return upload_card { file = file, area = area }
				end, { key = by_id, gap = theme.size.space_8 }),
			},
		})
	end

	local function joined_bars()
		return ui.show(joined, ui.column {
			mx = theme.size.space_8,
			shrink = 0,
			overflow = "hidden",
			rounded_tl = function()
				return if suggesting() then 0 else radius()
			end,
			rounded_tr = function()
				return if suggesting() then 0 else radius()
			end,
			bg = theme.color.bg_3,
			border = 1,
			border_sides = { 1, 1, 0, 1 },
			border_color = theme.color.border,
			ui.show(commanding, command_picker.hint(shared, area)),
			notice(),
			reply_bar(),
			edit_bar(),
			tray(),
		})
	end

	local function pick_files()
		if picking then
			return
		end
		picking = true
		ui.task(function()
			local ok, files = pcall(function()
				return ui.open_files({ title = "Upload a File", multiple = true }):await()
			end)
			picking = false
			if not ok then
				return
			end
			local paths = {}
			for _, file in files do
				table.insert(paths, file.path)
			end
			if #paths > 0 then
				actions.composer_add_files(paths, area)
			end
		end)
	end

	local function field_limit(): number
		if room == nil then
			return LINE * MAX_LINES
		end
		return math.max(LINE, room() / 2 - (chatbar() - LINE))
	end

	local function editor(text: any, box: any)
		local measured = ui.state(current.measure_key, LINE)
		local applied = 0
		local seen_pick = shared.pick:peek()
		local function caret(): number
			local element = box:peek()
			if element == nil then
				return #text:peek()
			end
			local _, focus = (element :: any):selection_bytes()
			return focus or #text:peek()
		end
		ui.effect(function()
			local typed = text:get()
			actions.composer_changed(typed, caret(), area)
		end)
		ui.effect(function()
			local replace = shared.replace:get()
			if replace.serial == applied then
				return
			end
			applied = replace.serial
			text:set(replace.text)
			local element = box:peek()
			if element ~= nil then
				(element :: any):set_selection_bytes(replace.caret)
				element:focus()
			end
		end)
		ui.effect(function()
			if shared.focus:get() == 0 then
				return
			end
			local element = box:peek()
			if element ~= nil then
				element:focus()
			end
		end)
		ui.effect(function()
			local serial = shared.pick:get()
			if serial ~= seen_pick then
				seen_pick = serial
				pick_files()
			end
		end)
		return ui.row {
			relative = true,
			grow = 1,
			min_w = 0,
			ml = theme.size.space_12,
			py = function()
				return (chatbar() - 2 - LINE) / 2
			end,
			ui.text {
				function()
					local typed = text:get()
					return if typed == "" or string.sub(typed, -1) == "\n" then typed .. " " else typed
				end,
				literal = true,
				absolute = true,
				left = 0,
				right = 0,
				top = 0,
				wrap = true,
				opacity = 0,
				pointer_events = "none",
				text_size = TEXT_SIZE,
				line_height = LINE,
				on_resize = function(size)
					measured:set(size.h)
				end,
			},
			ui.textarea {
				id = area .. "_field",
				value = text,
				ref = box,
				label = "Message",
				placeholder = shared.placeholder,
				placeholder_color = theme.color.text_5,
				spellcheck = true,
				wrap = true,
				w_full = true,
				h = function()
					return math.clamp(measured:get(), LINE, field_limit())
				end,
				px = 0,
				py = 0,
				bg = "#00000000",
				border = 0,
				focus = { border = 0 },
				text_size = TEXT_SIZE,
				line_height = LINE,
				color = theme.color.text_3,
				decorations = function()
					return actions.composer_backgrounds(text:get())
				end,
				on_key = function(event)
					return actions.composer_key(event, text:peek(), area)
				end,
			},
		}
	end

	local function composer_button(name: string, tooltip: string, on_click: any)
		return ui.row {
			mx = theme.size.space_4,
			icon_button {
				id = area .. "_" .. name .. "_button",
				name = name,
				icon_size = ICON,
				hit_size = BUTTON,
				tooltip = tooltip,
				on_click = on_click,
			},
		}
	end

	local function toolbar()
		local bar_element: any = nil
		local function opens(tab: string)
			if area ~= MAIN.area then
				return nil
			end
			return function()
				if bar_element ~= nil and bar_element.x ~= nil and bar_element.y ~= nil then
					actions.composer_picker(tab, {
						x = bar_element.x,
						y = bar_element.y,
						w = bar_element.width,
						h = bar_element.height,
					})
				end
			end
		end
		return ui.row {
			h = function()
				return chatbar() - 2
			end,
			pr = theme.size.space_8,
			items = "center",
			shrink = 0,
			on_resize = function(_, element)
				bar_element = element
			end,
			ui.show(function()
				return not commanding()
			end, ui.row {
				items = "center",
				plugin_slot { name = "composer.buttons", context = { area = area }, gap = 2 },
				composer_button("gif", "Open GIF picker", opens("gif")),
				composer_button("sticker", "Open sticker picker", opens("sticker")),
			}),
			composer_button("emoji", "Select emoji", opens("emoji")),
			ui.show(function()
				return not commanding()
			end, composer_button("apps", "Apps", opens("apps"))),
		}
	end

	local function counter(text: any)
		local function remaining(): number
			return shared.limit:get() - length(text:get())
		end
		return ui.show(function()
			return remaining() < COUNTER_FROM and not commanding()
		end, ui.text {
			function()
				return tostring(remaining())
			end,
			id = area .. "_counter",
			absolute = true,
			right = theme.number(theme.size.space_16),
			bottom = theme.number(theme.size.space_4),
			text_size = theme.size.font_size_xs,
			line_height = theme.size.line_height_xs,
			weight = theme.weight.medium,
			color = function()
				return if remaining() < 0 then theme.color.red_2 else theme.color.text_4
			end,
		})
	end

	local function typing_line()
		return ui.show(function()
			return not suggesting() and not picker_open() and not commanding()
		end, ui.row {
			w_full = true,
			h = theme.size.typing_height,
			shrink = 0,
			px = theme.size.space_16,
			gap = theme.size.space_6,
			items = "center",
			ui.show(function()
				return shared.typing:get() ~= ""
			end, typing_dots()),
			ui.text {
				function()
					return shared.typing:get()
				end,
				literal = true,
				grow = 1,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				color = theme.color.text_3,
			},
		})
	end
	local text = ui.state(current.text_key, "")
	local box = ui.ref()
	local function plus()
		if field_box == nil or field_box.x == nil or field_box.y == nil then
			return
		end
		actions.composer_plus({ x = field_box.x, y = field_box.y }, area)
	end
	return ui.show(shared.open, ui.column {
		relative = true,
		w_full = true,
		shrink = 0,
		ui.show(picker_open, command_picker.picker(shared, area)),
		ui.show(optional_open, command_picker.optional(shared, area)),
		ui.show(choices_open, command_picker.choices(shared, area)),
		ui.show(dropbox_open, command_picker.dropbox(shared, area)),
		popup(text, shared, area, suggesting),
		typing_line(),
		joined_bars(),
		ui.row {
			id = area .. "_box",
			relative = true,
			mx = theme.size.space_8,
			mb = theme.size.space_8,
			min_h = theme.size.chatbar_height,
			items = "start",
			bg = theme.color.bg_3,
			border = 1,
			border_color = theme.color.border_light,
			on_resize = function(_, element)
				field_box = element
				if area == MAIN.area then
					actions.report_slot("composer", element)
				end
			end,
			rounded_bl = radius,
			rounded_br = radius,
			rounded_tl = function()
				return if joined() or suggesting() then 0 else radius()
			end,
			rounded_tr = function()
				return if joined() or suggesting() then 0 else radius()
			end,
			ui.show(commanding, ui.row {
				h = function()
					return chatbar() - 2
				end,
				shrink = 0,
				command_picker.app_slot(shared),
			}, ui.row {
				w = LEADING,
				h = function()
					return chatbar() - 2
				end,
				shrink = 0,
				justify = "center",
				items = "center",
				icon_button {
					id = area .. "_plus_button",
					name = "plus",
					icon_size = ICON,
					hit_size = BUTTON,
					tooltip = "More",
					on_click = plus,
				},
			}),
			ui.show(commanding, ui.row {
				grow = 1,
				min_w = 0,
				h = function()
					return chatbar() - 2
				end,
				command_picker.editor(shared, area),
			}, editor(text, box)),
			toolbar(),
			counter(text),
		},
	}, ui.row {
		w_full = true,
		h = function()
			return theme.number(theme.size.typing_height)() + chatbar() + theme.number(theme.size.space_8)()
		end,
		shrink = 0,
	})
end
