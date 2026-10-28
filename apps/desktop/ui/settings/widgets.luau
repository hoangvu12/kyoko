local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")

local PHI = 1.618
local SWITCH_WIDTH = 36
local SWITCH_HEIGHT = 20
local SWITCH_THUMB = 16
local SWITCH_INSET = 2
local RADIO = 16
local RADIO_CHECK = 12
local RADIO_GAP = 12
local RADIO_ROW = 18
local SMALL_BUTTON = 24
local BUTTON = 32
local SLIDER_HIT = 20
local SLIDER_TRACK = 6
local SLIDER_THUMB = 16
local SLIDER_EMPTY = 0.25

local M = {}

function M.leading(size: string): () -> number
	local points = theme.number(size)
	return function()
		return points() * PHI
	end
end

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

function M.note(key: string): () -> string
	return function()
		return (state.settings.notes:get() or {})[key] or ""
	end
end

function M.label(text: any, size: string, color: any, extra: { [string]: any }?)
	local props: { [string]: any } = {
		text,
		text_size = size,
		line_height = M.leading(size),
		color = color,
	}
	for key, value in extra or {} do
		props[key] = value
	end
	return ui.text(props)
end

function M.heading(text: string, bottom: any?)
	return M.label(string.upper(text), theme.size.font_tiny, theme.color.header_secondary, {
		weight = theme.weight.bold,
		literal = true,
		pb = bottom or 0,
	})
end

function M.row(title: any, note: any?, control: any?)
	return ui.row {
		w_full = true,
		items = "center",
		justify = "between",
		gap = theme.size.space_16,
		py = theme.size.space_12,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.divider,
		ui.column {
			grow = 1,
			min_w = 0,
			M.label(title, theme.size.font_md, theme.color.header_primary, { weight = theme.weight.medium, literal = true }),
			if note
				then ui.show(function()
					return (if type(note) == "function" then note() else note) ~= ""
				end, M.label(note, theme.size.font_sm, theme.color.text_muted, { literal = true, wrap = true }))
				else nil,
		},
		if control then ui.div { shrink = 0, control } else nil,
	}
end

M.switch = ui.component(function(props: { checked: () -> boolean, on_change: (boolean) -> (), label: string?, id: string? })
	return ui.button {
		id = props.id,
		relative = true,
		w = SWITCH_WIDTH,
		h = SWITCH_HEIGHT,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		rounded = SWITCH_HEIGHT,
		role = "switch",
		label = props.label,
		checked = props.checked,
		cursor = "pointer",
		bg = function()
			return if props.checked() then theme.color.brand else theme.color.interactive_muted
		end,
		on_click = function()
			props.on_change(not props.checked())
		end,
		ui.div {
			absolute = true,
			top = SWITCH_INSET,
			left = function()
				return if props.checked() then SWITCH_WIDTH - SWITCH_THUMB - SWITCH_INSET else SWITCH_INSET
			end,
			w = SWITCH_THUMB,
			h = SWITCH_THUMB,
			rounded = SWITCH_THUMB,
			bg = theme.color.text_on_brand,
		},
	}
end)

function M.toggle(key: string, label: string?)
	return M.switch {
		checked = M.on(key),
		label = label,
		on_change = function(checked: boolean)
			M.set(key, checked)
		end,
	}
end

function M.toggle_row(key: string, title: string, note: any?)
	return M.row(title, note, M.toggle(key, title))
end

local function radio_mark(chosen: () -> boolean)
	return ui.row {
		w = RADIO,
		h = RADIO,
		shrink = 0,
		rounded = RADIO,
		justify = "center",
		items = "center",
		border = function()
			return if chosen() then 0 else 1
		end,
		border_color = theme.color.interactive_muted,
		bg = function()
			return if chosen() then theme.color.brand else theme.color.background_tertiary
		end,
		ui.show(chosen, icon { name = "check", size = RADIO_CHECK, color = theme.color.text_on_brand }),
	}
end

function M.radios(options: { { label: string, value: any } }, current: () -> any, choose: (any) -> ())
	local rows = {}
	for _, option in options do
		local function chosen(): boolean
			return current() == option.value
		end
		table.insert(
			rows,
			ui.button {
				h = RADIO_ROW,
				p = 0,
				bg = "#00000000",
				border = 0,
				focus = { border = 0 },
				gap = theme.size.space_8,
				items = "center",
				cursor = "pointer",
				role = "radio",
				checked = chosen,
				on_click = function()
					choose(option.value)
				end,
				radio_mark(chosen),
				M.label(option.label, theme.size.font_sm, theme.color.text_normal, { literal = true }),
			}
		)
	end
	return ui.column { gap = RADIO_GAP, items = "start", table.unpack(rows) }
end

function M.setting_radios(key: string, options: { { label: string, value: any } })
	return M.radios(options, function()
		return M.value(key)
	end, function(value: any)
		M.set(key, value)
	end)
end

M.slider = ui.component(function(props: {
	key: string,
	min: number,
	max: number,
	step: number,
	scale: number,
	reset: number?,
	tight: boolean?,
})
	local track = ui.ref()
	local dragging = ui.use_state(false)
	local held = ui.use_state(nil :: number?)
	local function stored(): number
		local found = M.value(props.key)
		return if type(found) == "number" then found else props.min
	end
	local function current(): number
		local value = held:get()
		return if value ~= nil then value else stored()
	end
	local function fraction(): number
		return math.clamp((current() - props.min) / (props.max - props.min), 0, 1)
	end
	local function pick(x: number)
		local width = track.width or 0
		if width <= 0 then
			return
		end
		local raw = props.min + math.clamp(x / width, 0, 1) * (props.max - props.min)
		local stepped = props.min + math.floor((raw - props.min) / props.step + 0.5) * props.step
		held:set(math.clamp(stepped, props.min, props.max))
	end
	local function commit()
		local value = held:get()
		dragging:set(false)
		if value ~= nil and math.abs(value - stored()) > props.step / 2 then
			M.set(props.key, value)
		end
		held:set(nil)
	end
	return ui.row {
		w_full = true,
		items = "center",
		gap = theme.size.space_16,
		py = if props.tight then 0 else theme.size.space_12,
		ui.div {
			ref = track,
			relative = true,
			grow = 1,
			h = SLIDER_HIT,
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
				top = (SLIDER_HIT - SLIDER_TRACK) / 2,
				h = SLIDER_TRACK,
				rounded = SLIDER_TRACK,
				bg = theme.color.brand,
				opacity = SLIDER_EMPTY,
			},
			ui.div {
				absolute = true,
				left = 0,
				top = (SLIDER_HIT - SLIDER_TRACK) / 2,
				w = function()
					return (track.width or 0) * fraction()
				end,
				h = SLIDER_TRACK,
				rounded = SLIDER_TRACK,
				bg = theme.color.brand,
			},
			ui.div {
				absolute = true,
				top = (SLIDER_HIT - SLIDER_THUMB) / 2,
				left = function()
					return (track.width or 0) * fraction() - SLIDER_THUMB / 2
				end,
				w = SLIDER_THUMB,
				h = SLIDER_THUMB,
				rounded = SLIDER_THUMB,
				bg = theme.color.text_on_brand,
			},
		},
		M.label(function()
			return string.format("%d%%", math.floor(current() * props.scale + 0.5))
		end, theme.size.font_sm, theme.color.text_muted, { w = 48, shrink = 0 }),
		if props.reset
			then M.button {
				text = "Reset",
				tone = "ghost",
				small = true,
				on_click = function()
					M.set(props.key, props.reset)
				end,
			}
			else nil,
	}
end)
M.button = ui.component(function(props: { text: string, tone: string?, small: boolean?, on_click: () -> (), id: string? })
	local hovered = ui.use_state(false)
	local tone = props.tone or "secondary"
	local function fill(): string
		local hover = hovered:get()
		if tone == "primary" then
			return if hover then theme.color.brand_hover else theme.color.brand
		elseif tone == "ghost" then
			return if hover then theme.color.modifier_hover else "#00000000"
		elseif tone == "danger" then
			return if hover then theme.color.danger_hover else theme.color.danger
		end
		return if hover then theme.color.button_secondary_hover else theme.color.button_secondary
	end
	local ink = if tone == "primary" or tone == "danger" then theme.color.text_on_brand else theme.color.text_3
	return ui.button {
		id = props.id,
		h = if props.small then SMALL_BUTTON else BUTTON,
		px = if props.small then theme.size.space_8 else theme.size.space_12,
		shrink = 0,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		justify = "center",
		items = "center",
		cursor = "pointer",
		bg = fill,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		ui.text { props.text, literal = true, text_size = theme.size.font_sm, color = ink },
	}
end)

function M.placeholder(name: string, note: string)
	return ui.row {
		w_full = true,
		items = "center",
		gap = theme.size.space_16,
		p = theme.size.space_16,
		rounded = theme.size.radius_md,
		bg = theme.color.background_secondary,
		icon { name = name, size = theme.size.icon_lg :: any, color = theme.color.interactive_muted },
		M.label(note, theme.size.font_md, theme.color.text_muted, { literal = true, wrap = true, grow = 1, min_w = 0 }),
	}
end

function M.hint(text: any)
	return M.label(text, theme.size.font_sm, theme.color.text_muted, { literal = true, wrap = true })
end

return M
