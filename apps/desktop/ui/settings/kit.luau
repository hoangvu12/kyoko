local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local avatar = require("@components/avatar")
local search = require("./search")

export type Option = { label: string, value: any, note: string? }

export type Row = {
	kind: string?,
	key: string?,
	title: string?,
	note: any?,
	keywords: string?,
	options: { Option }?,
	control: any?,
	build: (() -> any)?,
	checked: (() -> boolean)?,
	on_change: ((any) -> ())?,
	page: string?,
	icon: string?,
	min: number?,
	max: number?,
	step: number?,
	scale: number?,
	reset: number?,
	id: string?,
	visible: (() -> boolean)?,
}

export type Heading = {
	key: string,
	title: string?,
	keywords: string?,
	rows: { Row },
	action: any?,
}

export type Spec = { headings: { Heading } }

local CLEAR = "#00000000"

local M = {}

M.anchors = {} :: { [string]: any }

function M.value(key: string): any
	return (state.settings.values:get() or {})[key]
end

function M.on(key: string): () -> boolean
	return function()
		return M.value(key) == true
	end
end

function M.set(key: string, value: any)
	actions.set_setting(key, value)
end

function M.note_of(key: string): () -> string
	return function()
		return (state.settings.notes:get() or {})[key] or ""
	end
end

function M.note_or(key: string, fallback: string): () -> string
	local note = M.note_of(key)
	return function()
		local found = note()
		return if found ~= "" then found else fallback
	end
end

function M.text(content: any, size: string, line: string, color: any, extra: { [string]: any }?)
	local props: { [string]: any } = {
		content,
		literal = true,
		text_size = size,
		line_height = line,
		color = color,
	}
	for key, value in extra or {} do
		props[key] = value
	end
	return ui.text(props)
end

function M.label(title: any, note: any?)
	return ui.column {
		grow = 1,
		shrink = 1,
		min_w = 0,
		M.text(title, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_2, {
			weight = theme.weight.semibold,
			wrap = true,
		}),
		if note
			then ui.show(function()
				return (if type(note) == "function" then note() else note) ~= ""
			end, M.text(note, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, {
				wrap = true,
				mt = theme.size.space_2,
			}))
			else nil,
	}
end

function M.hint(content: any)
	return M.text(content, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, { wrap = true })
end

function M.caption(content: string)
	return M.text(content, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_2, {
		weight = theme.weight.semibold,
	})
end

function M.shown_value(content: any)
	return M.text(content, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_4, {
		truncate = true,
		min_w = 0,
	})
end

local function marked(row: Row, child: any)
	return ui.div {
		relative = true,
		w_full = true,
		mt = theme.size.settings_row_gap,
		ui.show(search.highlighted(row), ui.div {
			id = "settings_match_" .. (row.key or row.title or ""),
			absolute = true,
			left = -theme.number(theme.size.space_8)(),
			right = -theme.number(theme.size.space_8)(),
			top = -theme.number(theme.size.space_8)(),
			bottom = -theme.number(theme.size.space_8)(),
			pointer_events = "none",
			rounded = theme.size.radius_sm,
			bg = theme.color.mention,
		}),
		child,
	}
end

function M.line(title: any, note: any?, control: any?)
	return ui.row {
		w_full = true,
		items = if note then "start" else "center",
		gap = theme.size.space_16,
		M.label(title, note),
		if control then ui.div { shrink = 0, control } else nil,
	}
end

M.switch = ui.component(function(props: { checked: () -> boolean, on_change: (boolean) -> (), label: string?, id: string? })
	return ui.button {
		id = props.id,
		w = theme.size.switch_width,
		h = theme.size.switch_height,
		shrink = 0,
		p = 0,
		px = theme.size.space_4,
		rounded = theme.size.switch_height,
		role = "switch",
		label = props.label,
		checked = props.checked,
		cursor = "pointer",
		items = "center",
		justify = function()
			return if props.checked() then "end" else "start"
		end,
		border = theme.size.border_thickness,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		border_color = function()
			return if props.checked() then theme.color.button_border else theme.color.border
		end,
		bg = function()
			return if props.checked() then theme.color.accent_3 else theme.color.bg_3
		end,
		on_click = function()
			props.on_change(not props.checked())
		end,
		ui.div {
			w = theme.size.switch_thumb_size,
			h = theme.size.switch_thumb_size,
			shrink = 0,
			rounded = theme.size.switch_thumb_size,
			bg = function()
				return if props.checked() then theme.color.bg_4 else theme.color.text_3
			end,
		},
	}
end)

M.vc_switch = ui.component(function(props: { checked: () -> boolean, on_change: (boolean) -> (), label: string?, id: string? })
	return ui.button {
		id = props.id,
		w = theme.size.vc_switch_width,
		h = theme.size.switch_height,
		shrink = 0,
		p = 0,
		px = theme.size.space_2,
		border = 0,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		rounded = theme.size.switch_height,
		role = "switch",
		label = props.label,
		checked = props.checked,
		cursor = "pointer",
		items = "center",
		justify = function()
			return if props.checked() then "end" else "start"
		end,
		bg = function()
			return if props.checked() then theme.color.accent_2 else theme.color.bg_1
		end,
		on_click = function()
			props.on_change(not props.checked())
		end,
		ui.div {
			w = theme.size.vc_switch_thumb_size,
			h = theme.size.vc_switch_thumb_size,
			shrink = 0,
			rounded = theme.size.vc_switch_thumb_size,
			bg = function()
				return if props.checked() then theme.color.text_0 else theme.color.text_3
			end,
		},
	}
end)

local function fill(tone: string, hovered: boolean): string
	if tone == "primary" then
		return if hovered then theme.color.accent_4 else theme.color.accent_3
	elseif tone == "critical" then
		return if hovered then theme.color.red_4 else theme.color.red_3
	end
	return if hovered then theme.color.bg_1 else theme.color.bg_2
end

local function ink(tone: string): string
	if tone == "primary" or tone == "critical" then
		return theme.color.text_0
	elseif tone == "critical_secondary" then
		return theme.color.red_2
	end
	return theme.color.text_3
end

M.button = ui.component(function(props: {
	text: string,
	tone: string?,
	on_click: () -> (),
	id: string?,
	icon: string?,
	height: string?,
})
	local hovered = ui.use_state(false)
	local tone = props.tone or "secondary"
	local solid = tone == "primary" or tone == "critical"
	return ui.button {
		id = props.id,
		h = props.height or theme.size.button_height_sm,
		px = theme.size.space_16,
		gap = theme.size.space_8,
		shrink = 0,
		rounded = theme.size.radius_sm,
		border = theme.size.border_thickness,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		border_color = if solid then theme.color.button_border else theme.color.border_light,
		justify = "center",
		items = "center",
		cursor = "pointer",
		bg = function()
			return fill(tone, hovered:get())
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		if props.icon then icon { name = props.icon, size = theme.size.icon_size_sm :: any, color = ink(tone) } else nil,
		ui.text {
			props.text,
			literal = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = ink(tone),
		},
	}
end)

local function radio_mark(chosen: () -> boolean)
	return ui.div {
		w = theme.size.radio_size,
		h = theme.size.radio_size,
		shrink = 0,
		rounded = theme.size.radio_size,
		border = function()
			return if chosen() then theme.number(theme.size.radio_border_selected)() else theme.number(theme.size.border_thickness)()
		end,
		border_color = function()
			return if chosen() then theme.color.accent_2 else theme.color.border
		end,
		bg = function()
			return if chosen() then theme.color.text_1 else theme.color.bg_2
		end,
	}
end

function M.radios(options: { Option }, current: () -> any, choose: (any) -> (), prefix: string?)
	local rows = {}
	for index, option in options do
		local function chosen(): boolean
			return current() == option.value
		end
		table.insert(
			rows,
			ui.button {
				id = if prefix then prefix .. "_" .. tostring(index) else nil,
				w_full = true,
				p = 0,
				bg = CLEAR,
				border = 0,
				focus = { border = 0 },
				gap = theme.size.space_12,
				items = "center",
				cursor = "pointer",
				role = "radio",
				checked = chosen,
				on_click = function()
					choose(option.value)
				end,
				items = if option.note then "start" else "center",
				radio_mark(chosen),
				if option.note
					then ui.column {
						grow = 1,
						min_w = 0,
						gap = theme.size.space_2,
						M.text(option.label, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_2, {
							weight = theme.weight.medium,
							wrap = true,
							min_w = 0,
						}),
						M.hint(option.note),
					}
					else M.text(option.label, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_3, {
						wrap = true,
						min_w = 0,
					}),
			}
		)
	end
	return ui.column { w_full = true, gap = theme.size.space_12, table.unpack(rows) }
end

M.slider = ui.component(function(props: Row)
	local key = props.key or ""
	local low = props.min or 0
	local high = props.max or 1
	local step = props.step or 0.05
	local track = ui.ref()
	local dragging = ui.use_state(false)
	local held = ui.use_state(nil :: number?)
	local function stored(): number
		local found = M.value(key)
		return if type(found) == "number" then found else low
	end
	local function current(): number
		local value = held:get()
		return if value ~= nil then value else stored()
	end
	local function fraction(): number
		return math.clamp((current() - low) / (high - low), 0, 1)
	end
	local function pick(x: number)
		local width = track.width or 0
		if width <= 0 then
			return
		end
		local raw = low + math.clamp(x / width, 0, 1) * (high - low)
		local stepped = low + math.floor((raw - low) / step + 0.5) * step
		held:set(math.clamp(stepped, low, high))
	end
	local function commit()
		local value = held:get()
		dragging:set(false)
		if value ~= nil and math.abs(value - stored()) > step / 2 then
			M.set(key, value)
		end
		held:set(nil)
	end
	local grabber = theme.number(theme.size.slider_grabber_size)
	local bar = theme.number(theme.size.slider_track_height)
	return ui.row {
		w_full = true,
		items = "center",
		gap = theme.size.space_16,
		ui.div {
			ref = track,
			relative = true,
			grow = 1,
			h = theme.size.slider_grabber_size,
			cursor = "pointer",
			on_press = function(event: any)
				dragging:set(true)
				pick(event.local_x)
			end,
			on_pointer_move = function(event: any)
				if dragging:get() then
					pick(event.local_x)
				end
			end,
			on_release = commit,
			on_hover = function(inside: boolean)
				if not inside and dragging:get() then
					commit()
				end
			end,
			ui.div {
				absolute = true,
				left = 0,
				right = 0,
				top = function()
					return (grabber() - bar()) / 2
				end,
				h = theme.size.slider_track_height,
				rounded = theme.size.slider_track_height,
				bg = theme.color.bg_2,
			},
			ui.div {
				absolute = true,
				left = 0,
				top = function()
					return (grabber() - bar()) / 2
				end,
				w = function()
					return (track.width or 0) * fraction()
				end,
				h = theme.size.slider_track_height,
				rounded = theme.size.slider_track_height,
				bg = theme.color.accent_2,
			},
			ui.div {
				absolute = true,
				top = 0,
				left = function()
					return (track.width or 0) * fraction() - grabber() / 2
				end,
				w = theme.size.slider_grabber_size,
				h = theme.size.slider_grabber_size,
				rounded = theme.size.slider_grabber_size,
				bg = theme.color.text_3,
			},
		},
		M.text(function()
			return string.format("%d%%", math.floor(current() * (props.scale or 100) + 0.5))
		end, theme.size.font_size_sm, theme.size.line_height_label, theme.color.text_4, {
			w = theme.size.space_56,
			shrink = 0,
		}),
		if props.reset
			then M.button {
				text = "Reset",
				on_click = function()
					M.set(key, props.reset)
				end,
			}
			else nil,
	}
end)

M.range = ui.component(function(props: {
	id: string?,
	label: string,
	value: any,
	min: number,
	max: number,
	step: number?,
	marks: { string },
})
	local track = ui.ref()
	local dragging = ui.use_state(false)
	local step = props.step or 1
	local function fraction(): number
		if props.max <= props.min then
			return 0
		end
		return math.clamp((props.value:get() - props.min) / (props.max - props.min), 0, 1)
	end
	local function pick(x: number)
		local width = track.width or 0
		if width <= 0 then
			return
		end
		local raw = props.min + math.clamp(x / width, 0, 1) * (props.max - props.min)
		local stepped = props.min + math.floor((raw - props.min) / step + 0.5) * step
		props.value:set(math.clamp(stepped, props.min, props.max))
	end
	local grabber = theme.number(theme.size.slider_grabber_size)
	local bar = theme.number(theme.size.slider_track_height)
	local marks = {}
	for _, mark in props.marks do
		table.insert(
			marks,
			M.text(mark, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_4, {
				weight = theme.weight.medium,
				shrink = 0,
			})
		)
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_6,
		ui.row { w_full = true, justify = "between", table.unpack(marks) },
		ui.div {
			id = props.id,
			ref = track,
			relative = true,
			w_full = true,
			h = theme.size.slider_grabber_size,
			cursor = "pointer",
			label = props.label,
			on_press = function(event: any)
				dragging:set(true)
				pick(event.local_x)
			end,
			on_pointer_move = function(event: any)
				if dragging:get() then
					pick(event.local_x)
				end
			end,
			on_release = function()
				dragging:set(false)
			end,
			on_hover = function(inside: boolean)
				if not inside then
					dragging:set(false)
				end
			end,
			ui.div {
				absolute = true,
				left = 0,
				right = 0,
				top = function()
					return (grabber() - bar()) / 2
				end,
				h = theme.size.slider_track_height,
				rounded = theme.size.slider_track_height,
				bg = theme.color.bg_2,
			},
			ui.div {
				absolute = true,
				left = 0,
				top = function()
					return (grabber() - bar()) / 2
				end,
				w = function()
					return (track.width or 0) * fraction()
				end,
				h = theme.size.slider_track_height,
				rounded = theme.size.slider_track_height,
				bg = theme.color.accent_2,
			},
			ui.div {
				absolute = true,
				top = 0,
				left = function()
					return (track.width or 0) * fraction() - grabber() / 2
				end,
				w = theme.size.slider_grabber_size,
				h = theme.size.slider_grabber_size,
				rounded = theme.size.slider_grabber_size,
				bg = theme.color.text_1,
			},
		},
	}
end)

function M.notice(content: string, glyph: string?)
	return ui.row {
		w_full = true,
		items = if note then "start" else "center",
		gap = theme.size.space_12,
		px = theme.size.space_16,
		py = theme.size.space_12,
		rounded = theme.size.radius_sm,
		bg = theme.color.background_feedback_info,
		border = theme.size.border_thickness,
		border_color = theme.color.blue_2,
		icon { name = glyph or "info", size = theme.size.icon_size_md :: any, color = theme.color.blue_1 },
		M.text(content, theme.size.font_size_sm, theme.size.line_height_label, theme.color.text_3, {
			wrap = true,
			grow = 1,
			min_w = 0,
		}),
	}
end

function M.card(...: any)
	return ui.column {
		w_full = true,
		gap = theme.size.space_4,
		p = theme.size.space_16,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		...,
	}
end

M.related = ui.component(function(props: Row)
	local hovered = ui.use_state(false)
	return ui.button {
		w_full = true,
		items = "center",
		gap = theme.size.space_16,
		p = theme.size.space_16,
		rounded = theme.size.radius_sm,
		border = 0,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		cursor = "pointer",
		bg = function()
			return if hovered:get() then theme.color.bg_2 else theme.color.bg_3
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = function()
			actions.open_settings(props.page or "")
		end,
		ui.row {
			w = theme.size.avatar_size_md,
			h = theme.size.avatar_size_md,
			shrink = 0,
			items = "center",
			justify = "center",
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_2,
			icon { name = props.icon or "settings", size = theme.size.icon_size_md :: any, color = theme.color.text_3 },
		},
		M.label(props.title or "", props.note),
		icon { name = "chevron_right", size = theme.size.icon_size_md :: any, color = theme.color.text_4 },
	}
end)

local function toggle_of(row: Row)
	local checked = row.checked or M.on(row.key or "")
	return M.switch {
		id = row.id,
		label = row.title,
		checked = checked,
		on_change = function(on: boolean)
			if row.on_change then
				row.on_change(on)
			else
				M.set(row.key or "", on)
			end
		end,
	}
end

local function build_row(row: Row): any
	local kind = row.kind or "toggle"
	if kind == "toggle" then
		return M.line(row.title, row.note, toggle_of(row))
	elseif kind == "radios" then
		return ui.column {
			w_full = true,
			gap = theme.size.space_16,
			if row.title then M.label(row.title, row.note) else nil,
			M.radios(row.options or {}, function()
				return M.value(row.key or "")
			end, function(value: any)
				if row.on_change then
					row.on_change(value)
				else
					M.set(row.key or "", value)
				end
			end, row.id),
		}
	elseif kind == "slider" then
		return ui.column {
			w_full = true,
			gap = theme.size.space_12,
			M.label(row.title, row.note),
			M.slider(row),
		}
	elseif kind == "related" then
		return ui.column {
			w_full = true,
			gap = theme.size.space_8,
			M.caption("Related Settings"),
			M.related(row),
		}
	elseif kind == "block" then
		return if row.build then row.build() else ui.div {}
	end
	local control = row.control
	return M.line(row.title, row.note, if type(control) == "function" then control() else control)
end

function M.section(heading: Heading)
	local anchor = ui.ref()
	M.anchors[heading.key] = anchor
	local rows = {}
	for index, row in heading.rows do
		local built = marked(row, build_row(row))
		if not heading.title and index == 1 then
			built = ui.div { w_full = true, mt = -theme.number(theme.size.settings_row_gap)(), built }
		end
		table.insert(rows, if row.visible then ui.show(row.visible, built) else built)
	end
	return ui.column {
		ref = anchor,
		id = "settings_section_" .. heading.key,
		w_full = true,
		if heading.title
			then ui.row {
				w_full = true,
				items = "center",
				justify = "between",
				gap = theme.size.space_16,
				M.text(heading.title, theme.size.font_size_xl, theme.size.line_height_xl, theme.color.text_2, {
					weight = theme.weight.semibold,
				}),
				heading.action,
			}
			else nil,
		table.unpack(rows),
	}
end

function M.divider()
	return ui.div {
		w_full = true,
		h = theme.size.border_thickness,
		shrink = 0,
		mt = theme.size.settings_divider_margin,
		mb = theme.size.settings_divider_bottom,
		bg = theme.color.border,
	}
end

function M.page(spec: Spec)
	table.clear(M.anchors)
	local parts = {}
	for index, heading in spec.headings do
		if index > 1 then
			table.insert(parts, M.divider())
		end
		table.insert(parts, M.section(heading))
	end
	return ui.column { w_full = true, table.unpack(parts) }
end

function M.title(content: any, action: any?)
	return ui.row {
		w_full = true,
		items = "center",
		justify = "between",
		gap = theme.size.space_16,
		M.text(content, theme.size.font_size_xl, theme.size.line_height_xl, theme.color.text_2, {
			weight = theme.weight.semibold,
			min_w = 0,
			truncate = true,
		}),
		action,
	}
end

function M.eyebrow(content: any)
	return M.text(function()
		return string.upper(if type(content) == "function" then content() else content)
	end, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_4, {
		weight = theme.weight.bold,
		shrink = 0,
	})
end

function M.field(caption: string, content: any)
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		M.eyebrow(caption),
		content,
	}
end

local function input_props(extra: { [string]: any }?): { [string]: any }
	local props: { [string]: any } = {
		w_full = true,
		h = theme.size.input_height,
		px = theme.size.space_12,
		rounded = theme.size.radius_sm,
		border = theme.size.border_thickness,
		border_color = theme.color.border_light,
		hover = { border_color = theme.color.border },
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		bg = theme.color.bg_3,
		text_size = theme.size.font_size_md,
		color = theme.color.text_3,
		placeholder_color = theme.color.text_5,
	}
	for key, value in extra or {} do
		props[key] = value
	end
	return props
end

function M.input(value: any, placeholder: string, extra: { [string]: any }?)
	local props = input_props(extra)
	props.value = value
	props.placeholder = placeholder
	props.label = props.label or placeholder
	return ui.input(props)
end

function M.textarea(value: any, placeholder: string, lines: number, extra: { [string]: any }?)
	local props = input_props(extra)
	props.value = value
	props.placeholder = placeholder
	props.label = props.label or placeholder
	props.wrap = true
	props.h = lines * theme.number(theme.size.line_height_label)() + theme.number(theme.size.space_10)() * 2
	props.py = theme.size.space_10
	props.line_height = theme.size.line_height_label
	return ui.textarea(props)
end

function M.search_input(value: any, placeholder: string, extra: { [string]: any }?)
	local props = input_props(extra)
	props.value = value
	props.placeholder = placeholder
	props.label = placeholder
	props.grow = 1
	props.min_w = 0
	props.h = theme.size.select_height
	props.pr = theme.size.space_40
	return ui.div {
		relative = true,
		grow = 1,
		min_w = 0,
		ui.input(props),
		ui.row {
			absolute = true,
			top = 0,
			bottom = 0,
			right = theme.number(theme.size.space_12)(),
			items = "center",
			pointer_events = "none",
			icon { name = "search", size = theme.size.icon_size_sm :: any, color = theme.color.text_4 },
		},
	}
end

M.select = ui.component(function(props: {
	labels: { string },
	current: () -> number,
	pick: (number) -> (),
	id: string?,
	w: any?,
	grow: number?,
	prefix: string?,
})
	local open = ui.use_state(false)
	local items = {}
	for index, text in props.labels do
		table.insert(
			items,
			ui.menu_item {
				text,
				id = if props.id then props.id .. "_" .. tostring(index) else nil,
				h = theme.size.settings_item_height,
				checked = function()
					return props.current() == index
				end,
				on_click = function()
					props.pick(index)
				end,
			}
		)
	end
	local menu = ui.menu {
		open = open,
		label = "Choose",
		match_anchor_width = true,
		px = theme.size.space_8,
		py = theme.size.space_6,
		gap = theme.size.space_2,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		placement = { side = "bottom", align = "start", offset = 4 },
		anchor = ui.button {
			id = props.id,
			w_full = true,
			min_w = 0,
			h = theme.size.select_height,
			px = theme.size.space_12,
			gap = theme.size.space_8,
			shrink = 0,
			items = "center",
			rounded = theme.size.radius_sm,
			border = theme.size.border_thickness,
			border_color = theme.color.border_light,
			hover = { border_color = theme.color.border },
			focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
			bg = theme.color.bg_3,
			cursor = "pointer",
			on_click = ui.toggle(open),
			M.text(function()
				return (props.prefix or "") .. (props.labels[props.current()] or "")
			end, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_3, {
				grow = 1,
				min_w = 0,
				truncate = true,
			}),
			icon { name = "chevron_down", size = theme.size.icon_size_md :: any, color = theme.color.text_3 },
		},
		table.unpack(items),
	}
	return ui.div {
		w = props.w,
		grow = props.grow,
		basis = if props.grow then 0 else nil,
		shrink = if props.w then 0 else 1,
		min_w = 0,
		menu,
	}
end)

export type Column = { title: string, grow: number?, w: any? }

local function cell(column: Column, content: any)
	return ui.row {
		grow = column.grow or 0,
		basis = if column.grow then 0 else nil,
		w = column.w,
		shrink = if column.w then 0 else 1,
		min_w = 0,
		items = "center",
		gap = theme.size.space_8,
		content,
	}
end

function M.table_head(columns: { Column }, extra: { [string]: any }?)
	local cells = {}
	for _, column in columns do
		table.insert(cells, cell(column, if column.title ~= "" then M.eyebrow(column.title) else nil))
	end
	local props: { [string]: any } = {
		w_full = true,
		gap = theme.size.space_12,
		px = theme.size.space_8,
		pb = theme.size.space_8,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.border,
	}
	for key, value in extra or {} do
		props[key] = value
	end
	for _, made in cells do
		table.insert(props, made)
	end
	return ui.row(props)
end

M.table_row = ui.component(function(props: {
	columns: { Column },
	cells: { any },
	id: string?,
	label: string?,
	on_click: (() -> ())?,
	on_press: ((any) -> ())?,
	context_menu: ((any) -> any)?,
	actions: ((() -> boolean) -> any)?,
	height: any?,
})
	local hovered = ui.use_state(false)
	local function hover(): boolean
		return hovered:get()
	end
	local body: { [string]: any } = {
		id = props.id,
		label = props.label,
		relative = true,
		w_full = true,
		min_h = props.height or theme.size.space_56,
		gap = theme.size.space_12,
		p = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		border = 0,
		border_sides = { 0, 0, 1, 0 },
		border_color = function()
			return if hovered:get() then CLEAR else theme.color.border
		end,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		bg = function()
			return if hovered:get() then theme.color.hover else CLEAR
		end,
		cursor = if props.on_click or props.on_press then "pointer" else nil,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		on_press = props.on_press,
		context_menu = props.context_menu,
	}
	for index, column in props.columns do
		table.insert(body, cell(column, props.cells[index]))
	end
	if props.actions then
		table.insert(body, props.actions(hover))
	end
	if props.on_click or props.on_press then
		return ui.button(body)
	end
	return ui.row(body)
end)

function M.round_button(props: {
	name: string,
	tooltip: string,
	on_click: (() -> ())?,
	on_press: ((any) -> ())?,
	tint: any?,
	id: string?,
	size: any?,
	bg: any?,
})
	return ui.button {
		id = props.id,
		w = props.size or theme.size.space_24,
		h = props.size or theme.size.space_24,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		rounded = props.size or theme.size.space_24,
		justify = "center",
		items = "center",
		cursor = "pointer",
		tooltip = props.tooltip,
		label = props.tooltip,
		bg = props.bg or theme.color.bg_4,
		hover = { bg = theme.color.bg_2 },
		on_click = props.on_click,
		on_press = props.on_press,
		icon { name = props.name, size = theme.size.icon_size_sm :: any, color = props.tint or theme.color.text_3 },
	}
end

function M.row_actions(shown: () -> boolean, ...: any)
	return ui.show(shown, ui.row {
		absolute = true,
		top = 0,
		bottom = 0,
		right = theme.number(theme.size.space_8)(),
		items = "center",
		gap = theme.size.space_8,
		...,
	})
end

function M.person(src: string, name: string, color: string?, sub: string?, size: string?)
	local side = theme.number(size or theme.size.avatar_size_sm)()
	return ui.row {
		min_w = 0,
		items = "center",
		gap = theme.size.space_8,
		ui.div {
			w = side,
			h = side,
			shrink = 0,
			rounded = side,
			bg = theme.color.bg_2,
			if src ~= "" then avatar { src = src, size = side } else nil,
		},
		ui.column {
			min_w = 0,
			M.text(name, theme.size.font_size_sm, theme.size.line_height_sm, if color and color ~= "" then color else theme.color.text_2, {
				weight = theme.weight.semibold,
				truncate = true,
				min_w = 0,
			}),
			if sub and sub ~= ""
				then M.text(sub, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_4, {
					truncate = true,
					min_w = 0,
				})
				else nil,
		},
	}
end

function M.chip(name: string, color: string?)
	return ui.row {
		h = theme.size.space_20,
		shrink = 0,
		px = theme.size.space_6,
		gap = theme.size.space_4,
		items = "center",
		rounded = theme.size.radius_xs,
		bg = theme.color.bg_2,
		ui.div {
			w = theme.size.space_8,
			h = theme.size.space_8,
			shrink = 0,
			rounded = theme.size.space_8,
			bg = if color and color ~= "" then color else theme.color.text_4,
		},
		M.text(name, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_3, {
			weight = theme.weight.medium,
		}),
	}
end

function M.save_bar(on_reset: () -> (), on_save: () -> ())
	return ui.row {
		id = "settings_save_bar",
		absolute = true,
		left = 0,
		right = 0,
		bottom = theme.number(theme.size.space_16)(),
		justify = "center",
		px = theme.size.space_24,
		ui.row {
			grow = 1,
			max_w = theme.size.settings_column_width,
			items = "center",
			gap = theme.size.space_12,
			py = theme.size.space_10,
			pr = theme.size.space_10,
			pl = theme.size.space_16,
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border,
			M.text("Careful — you have unsaved changes!", theme.size.font_size_md, theme.size.line_height_label, theme.color.text_3, {
				weight = theme.weight.medium,
				grow = 1,
				min_w = 0,
				wrap = true,
			}),
			ui.button {
				id = "settings_save_reset",
				h = theme.size.button_height_md,
				px = theme.size.space_16,
				border = 0,
				focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
				bg = CLEAR,
				cursor = "pointer",
				hover = { underline = true },
				on_click = on_reset,
				M.text("Reset", theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_3, {
					weight = theme.weight.medium,
				}),
			},
			M.button {
				id = "settings_save_changes",
				text = "Save Changes",
				tone = "primary",
				height = theme.size.button_height_md,
				on_click = on_save,
			},
		},
	}
end

function M.empty(glyph: string, content: string)
	return ui.column {
		w_full = true,
		items = "center",
		gap = theme.size.space_8,
		py = theme.size.space_40,
		icon { name = glyph, size = theme.size.icon_size_lg :: any, color = theme.color.text_5 },
		M.hint(content),
	}
end

function M.web_note(content: string)
	return {
		kind = "block",
		title = content,
		keywords = "discord web app browser",
		build = function()
			return ui.column {
				w_full = true,
				gap = theme.size.space_16,
				items = "start",
				M.notice(content),
				M.button {
					text = "Open Discord",
					icon = "external_link",
					on_click = function()
						actions.settings_action("open_web")
					end,
				},
			}
		end,
	}
end

return M
