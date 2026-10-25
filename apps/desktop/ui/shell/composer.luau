local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")

local FIELD = 44
local LEADING = 56
local LINE = 20
local TEXT_INSET = 10
local MAX_LINES = 10
local BAR = 32
local CHIP = 36
local CHIP_MAX = 240
local ROW = 36
local TYPING = 24
local COUNTER_FROM = 200

local function by_key(item: { key: string })
	return item.key
end

local function by_id(item: { id: string })
	return item.id
end

local function length(text: string): number
	return utf8.len(text) or #text
end

local function suggestion_visual(item: state.ComposerSuggestion)
	if item.kind == "glyph" then
		return ui.text { item.glyph, literal = true, text_size = 20, w = 24, align = "center", shrink = 0 }
	elseif item.kind == "icon" then
		return icon { name = item.icon, size = 20, color = theme.color.text_muted }
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

local function suggestion_row(item: state.ComposerSuggestion, index: number, text: any)
	return ui.row {
		h = ROW,
		px = theme.size.space_8,
		gap = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		bg = function()
			return if state.composer.selected:get() == index then theme.color.modifier_selected else "#00000000"
		end,
		hover = { bg = theme.color.modifier_hover },
		on_click = function()
			actions.composer_choose(index, text:peek())
		end,
		suggestion_visual(item),
		ui.text {
			item.label,
			literal = true,
			min_w = 0,
			shrink = 0,
			truncate = true,
			text_size = theme.size.font_md,
			weight = theme.weight.medium,
			color = theme.color.text_normal,
		},
		ui.show(item.detail ~= "", ui.text {
			item.detail,
			literal = true,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_sm,
			color = theme.color.text_muted,
		}),
	}
end

local function popup(text: any)
	return ui.show(function()
		return #state.composer.suggestions:get() > 0
	end, ui.column {
		w_full = true,
		mb = theme.size.space_8,
		p = theme.size.space_8,
		rounded = theme.size.radius_md,
		border = 1,
		border_color = theme.color.divider,
		bg = theme.color.background_floating,
		color = theme.color.text_normal,
		ui.each(state.composer.suggestions, function(item: state.ComposerSuggestion, index: number)
			return suggestion_row(item, index, text)
		end, { key = by_key, w_full = true }),
	})
end

local function notice_button(label: string, on_click: () -> (), danger: boolean)
	return ui.button {
		label,
		h = 24,
		px = theme.size.space_8,
		py = 0,
		rounded = theme.size.radius_sm,
		border = 0,
		focus = { border = 0 },
		text_size = theme.size.font_sm,
		weight = theme.weight.medium,
		color = theme.color.text_on_brand,
		bg = if danger then theme.color.danger else theme.color.brand,
		hover = { bg = if danger then theme.color.danger_hover else theme.color.brand_hover },
		on_click = on_click,
	}
end

local function notice()
	local current = state.composer.notice
	local function kind(): string
		return current:get().kind
	end
	return ui.show(function()
		return kind() ~= ""
	end, ui.row {
		w_full = true,
		min_h = BAR,
		mb = theme.size.space_8,
		px = theme.size.space_12,
		py = theme.size.space_4,
		gap = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_md,
		bg = theme.color.background_secondary,
		icon {
			name = "warning",
			size = 16,
			color = function()
				return if kind() == "lock" then theme.color.warning else theme.color.danger
			end,
		},
		ui.text {
			function()
				return current:get().text or ""
			end,
			literal = true,
			grow = 1,
			min_w = 0,
			wrap = true,
			text_size = theme.size.font_sm,
			color = function()
				return if kind() == "failed" then theme.color.danger else theme.color.text_normal
			end,
		},
		ui.show(function()
			return kind() == "lock" and current:get().confirmable == true
		end, notice_button("I understand", function()
			actions.composer_confirm()
		end, false)),
		ui.show(function()
			return kind() == "failed"
		end, ui.row {
			gap = theme.size.space_8,
			notice_button("Retry", function()
				actions.composer_retry(current:peek().nonce or "")
			end, false),
			notice_button("Delete", function()
				actions.composer_discard(current:peek().nonce or "")
			end, true),
		}),
		ui.show(function()
			return kind() == "refused"
		end, icon_button {
			name = "close",
			icon_size = 16,
			hit_size = 24,
			tooltip = "Dismiss",
			on_click = function()
				actions.composer_dismiss()
			end,
		}),
	})
end

local function top_bar(label: () -> string, extra: any, on_close: () -> (), tooltip: string)
	return ui.row {
		w_full = true,
		h = BAR,
		px = theme.size.space_12,
		items = "center",
		justify = "between",
		rounded_tl = theme.number(theme.size.radius_md),
		rounded_tr = theme.number(theme.size.radius_md),
		bg = theme.color.background_secondary,
		text_size = theme.size.font_sm,
		color = theme.color.text_muted,
		ui.text { label, literal = true, min_w = 0, truncate = true },
		ui.row {
			gap = theme.size.space_8,
			items = "center",
			shrink = 0,
			extra,
			icon_button {
				name = "close",
				icon_size = 16,
				hit_size = 24,
				tooltip = tooltip,
				on_click = on_close,
			},
		},
	}
end

local function reply_bar()
	local reply = state.composer.reply
	local ping = ui.show(function()
		return reply:get().own ~= true
	end, ui.button {
		h = 24,
		px = theme.size.space_4,
		py = 0,
		gap = theme.size.space_2,
		bg = "#00000000",
		border = 0,
		focus = { border = 0 },
		cursor = "pointer",
		tooltip = "Mention the author",
		text_size = theme.size.font_sm,
		weight = theme.weight.bold,
		color = function()
			return if reply:get().ping then theme.color.link else theme.color.text_muted
		end,
		on_click = function()
			actions.composer_toggle_ping()
		end,
		icon { name = "at_sign", size = 16 },
		ui.text(function()
			return if reply:get().ping then "ON" else "OFF"
		end),
	})
	return ui.show(function()
		return reply:get().active
	end, top_bar(function()
		return "Replying to " .. (reply:get().name or "")
	end, ping, function()
		actions.composer_cancel_reply()
	end, "Cancel reply"))
end

local function edit_bar()
	return ui.show(function()
		return state.composer.editing:get()
	end, top_bar(function()
		return "Editing message · escape to cancel · enter to save"
	end, nil, function()
		actions.composer_cancel_edit()
	end, "Cancel edit"))
end

local function file_chip(file: state.ComposerFile)
	return ui.row {
		max_w = CHIP_MAX,
		h = CHIP,
		pl = theme.size.space_8,
		pr = theme.size.space_4,
		gap = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		bg = theme.color.background_secondary,
		ui.show(
			file.image ~= "",
			ui.image { src = file.image, w = 28, h = 28, rounded = theme.size.radius_sm, fit = "cover", shrink = 0 },
			icon { name = "file", size = 20, color = theme.color.interactive_normal }
		),
		ui.column {
			min_w = 0,
			ui.text {
				file.name,
				literal = true,
				truncate = true,
				text_size = theme.size.font_sm,
				color = theme.color.text_normal,
			},
			ui.text { file.size, literal = true, text_size = theme.size.font_tiny, color = theme.color.text_muted },
		},
		icon_button {
			name = "close",
			icon_size = 16,
			hit_size = 24,
			tooltip = "Remove",
			on_click = function()
				actions.composer_remove_file(file.id)
			end,
		},
	}
end

local function chips()
	return ui.show(function()
		return #state.composer.files:get() > 0
	end, ui.row {
		w_full = true,
		flex_wrap = true,
		gap = theme.size.space_8,
		p = theme.size.space_8,
		ui.each(state.composer.files, function(file: state.ComposerFile)
			return file_chip(file)
		end, { key = by_id, w_full = true, flex_wrap = true, gap = theme.size.space_8 }),
	})
end

local picking = false

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
			actions.composer_add_files(paths)
		end
	end)
end

local function editor(text: any, box: any)
	local measured = ui.state("composer_measured", LINE)
	local applied = 0
	local function caret(): number
		local element = box:peek()
		if element == nil then
			return #text:peek()
		end
		local _, focus = (element :: any):selection_bytes()
		return focus or #text:peek()
	end
	ui.effect(function()
		local current = text:get()
		actions.composer_changed(current, caret())
	end)
	ui.effect(function()
		local replace = state.composer.replace:get()
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
		if state.composer.focus:get() == 0 then
			return
		end
		local element = box:peek()
		if element ~= nil then
			element:focus()
		end
	end)
	return ui.row {
		relative = true,
		grow = 1,
		min_w = 0,
		min_h = FIELD,
		py = 12,
		pl = TEXT_INSET,
		items = "center",
		ui.text {
			function()
				local current = text:get()
				return if current == "" or string.sub(current, -1) == "\n" then current .. " " else current
			end,
			literal = true,
			absolute = true,
			left = TEXT_INSET,
			right = 0,
			top = 0,
			wrap = true,
			opacity = 0,
			pointer_events = "none",
			text_size = theme.size.font_sm,
			line_height = LINE,
			on_resize = function(size)
				measured:set(size.h)
			end,
		},
		ui.textarea {
			value = text,
			ref = box,
			label = "Message",
			placeholder = state.composer.placeholder,
			placeholder_color = theme.color.text_muted,
			spellcheck = true,
			wrap = true,
			w_full = true,
			h = function()
				return math.clamp(measured:get(), LINE, LINE * MAX_LINES)
			end,
			px = 0,
			py = 0,
			bg = "#00000000",
			border = 0,
			focus = { border = 0 },
			text_size = theme.size.font_sm,
			line_height = LINE,
			color = theme.color.text_normal,
			on_key = function(event)
				return actions.composer_key(event, text:peek())
			end,
		},
	}
end

local function toolbar()
	return ui.row {
		h = FIELD,
		pr = theme.size.space_8,
		gap = 2,
		items = "center",
		shrink = 0,
		icon_button { name = "gif", icon_size = 24, hit_size = 32, tooltip = "Open GIF picker" },
		icon_button { name = "sticker", icon_size = 24, hit_size = 32, tooltip = "Open sticker picker" },
		icon_button { name = "emoji", icon_size = 24, hit_size = 32, tooltip = "Select emoji" },
	}
end

local function footer(text: any)
	local function remaining(): number
		return state.composer.limit:get() - length(text:get())
	end
	return ui.row {
		w_full = true,
		h = TYPING,
		px = theme.size.space_4,
		items = "center",
		ui.text {
			function()
				return state.composer.typing:get()
			end,
			literal = true,
			grow = 1,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_tiny,
			weight = theme.weight.semibold,
			color = theme.color.text_normal,
		},
		ui.show(function()
			return remaining() < COUNTER_FROM
		end, ui.text {
			function()
				return tostring(remaining())
			end,
			shrink = 0,
			text_size = theme.size.font_tiny,
			weight = theme.weight.semibold,
			color = function()
				return if remaining() < 0 then theme.color.danger else theme.color.text_muted
			end,
		}),
	}
end

return function()
	local radius = theme.number(theme.size.radius_md)
	local text = ui.state("composer_text", "")
	local box = ui.ref()
	local has_bar = function(): boolean
		return state.composer.reply:get().active or state.composer.editing:get()
	end
	return ui.show(state.composer.open, ui.column {
		relative = true,
		w_full = true,
		shrink = 0,
		px = theme.size.space_16,
		bg = theme.color.background_primary,
		popup(text),
		notice(),
		reply_bar(),
		edit_bar(),
		ui.column {
			relative = true,
			w_full = true,
			bg = theme.color.input_background,
			rounded_bl = theme.number(theme.size.radius_md),
			rounded_br = theme.number(theme.size.radius_md),
			rounded_tl = function()
				return if has_bar() then 0 else radius()
			end,
			rounded_tr = function()
				return if has_bar() then 0 else radius()
			end,
			chips(),
			ui.row {
				w_full = true,
				min_h = FIELD,
				items = "end",
				ui.row {
					w = LEADING,
					h = FIELD,
					shrink = 0,
					justify = "center",
					items = "center",
					icon_button {
						name = "plus",
						icon_size = 24,
						hit_size = 32,
						tooltip = "Upload a File",
						on_click = pick_files,
					},
				},
				editor(text, box),
				toolbar(),
			},
			ui.show(state.composer.drop_hover, ui.row {
				absolute = true,
				left = 0,
				right = 0,
				top = 0,
				bottom = 0,
				rounded = theme.size.radius_md,
				bg = theme.color.brand,
				opacity = 0.12,
				pointer_events = "none",
			}),
		},
		footer(text),
	}, ui.row { w_full = true, h = FIELD + TYPING, shrink = 0, bg = theme.color.background_primary })
end
