local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local dialog = require("../settings/dialog")
local kit = require("../settings/kit")

local COLUMNS = 3
local TILE_RATIO = 9 / 16
local PICKED_CHROME = 300
local TABS = {
	{ key = "window", label = "Applications", empty = "No windows to share." },
	{ key = "screen", label = "Screens", empty = "No screens to share." },
}
local RESOLUTIONS = {
	{ key = "720", label = "720p" },
	{ key = "1080", label = "1080p" },
	{ key = "source", label = "Source" },
}
local FRAME_RATES = {
	{ key = 15, label = "15 FPS" },
	{ key = 30, label = "30 FPS" },
	{ key = 60, label = "60 FPS" },
}

local function sources(): state.ShareSources
	return state.voice.screens:get()
end

local function preview(): state.ScreenPreview
	return state.voice.screen_preview:get()
end

local function n(token: string): number
	return theme.number(token)()
end

local function content_width(): number
	return n(theme.size.modal_width_lg) - n(theme.size.space_16) * 2 - n(theme.size.border_thickness) * 2
end

local function preview_height(): number
	local window = dialog.window
	local tall = if window then window.height or 0 else 0
	local room = tall - n(theme.size.gap) * 2 - PICKED_CHROME
	return math.max(0, math.min(content_width() * TILE_RATIO, room))
end

local segment = ui.component(function(props: {
	id: string,
	label: string,
	chosen: () -> boolean,
	on_click: () -> (),
	focused: boolean,
})
	local hovered = ui.use_state(false)
	return ui.button {
		id = props.id,
		h = theme.size.button_height_sm,
		px = theme.size.space_12,
		grow = 1,
		basis = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		justify = "center",
		items = "center",
		cursor = "pointer",
		bg = function()
			if props.chosen() then
				return theme.color.bg_1
			end
			return if hovered:get() then theme.color.hover else "#00000000"
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		on_mount = function(element: any)
			if props.focused and props.chosen() then
				element:focus()
			end
		end,
		ui.text {
			props.label,
			literal = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = function()
				return if props.chosen() then theme.color.text_2 else theme.color.text_4
			end,
		},
	}
end)

local function segments(
	id: string,
	options: { any },
	chosen: (any) -> boolean,
	choose: (any) -> (),
	focused: boolean
)
	local items = {}
	for _, option in options do
		table.insert(
			items,
			segment {
				id = id .. "_" .. tostring(option.key),
				label = option.label,
				chosen = function()
					return chosen(option.key)
				end,
				on_click = function()
					choose(option.key)
				end,
				focused = focused,
			}
		)
	end
	return ui.row {
		id = id,
		w_full = true,
		gap = theme.size.space_4,
		p = theme.size.space_4,
		rounded = theme.size.radius_md,
		bg = theme.color.bg_3,
		table.unpack(items),
	}
end

local source_tile = ui.component(function(props: { source: state.ShareSource, width: () -> number })
	local hovered = ui.use_state(false)
	local source = props.source
	return ui.column {
		id = "go_live_source_" .. source.id,
		w = props.width,
		gap = theme.size.space_8,
		cursor = "pointer",
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = function()
			local now = preview()
			actions.preview_screen(source.id, now.resolution, now.fps)
		end,
		ui.div {
			relative = true,
			w = props.width,
			h = function()
				return props.width() * TILE_RATIO
			end,
			rounded = theme.size.radius_md,
			overflow = "hidden",
			bg = theme.color.bg_2,
			border = theme.size.border_thickness,
			border_color = function()
				return if hovered:get() then theme.color.accent_2 else theme.color.border
			end,
			justify = "center",
			items = "center",
			icon {
				name = if source.kind == "screen" then "screen_share" else "app_window",
				size = n(theme.size.icon_size_lg),
				color = theme.color.text_5,
			},
			ui.image {
				id = "go_live_thumb_" .. source.id,
				src = "host:screen/" .. source.id,
				absolute = true,
				left = 0,
				top = 0,
				w_full = true,
				h_full = true,
				fit = "contain",
			},
		},
		ui.text {
			source.name,
			literal = true,
			w_full = true,
			truncate = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.medium,
			color = function()
				return if hovered:get() then theme.color.text_2 else theme.color.text_3
			end,
		},
	}
end)

local source_grid = ui.component(function(props: { tab: any })
	local function width(): number
		local gap = n(theme.size.space_16)
		return math.max(0, (content_width() - gap * (COLUMNS - 1)) / COLUMNS)
	end
	local shown = ui.computed(function(): { state.ShareSource }
		local kind = props.tab:get()
		local items = {}
		for _, source in sources().items do
			if source.kind == kind then
				table.insert(items, source)
			end
		end
		return items
	end)
	local function empty_text(): string
		if sources().loading then
			return "Looking for things to share..."
		end
		for _, tab in TABS do
			if tab.key == props.tab:get() then
				return tab.empty
			end
		end
		return ""
	end
	return ui.column {
		w_full = true,
		min_h = function()
			return width() * TILE_RATIO
		end,
		ui.show(
			function()
				return #shown:get() == 0
			end,
			ui.column {
				w_full = true,
				py = theme.size.space_32,
				items = "center",
				kit.text(empty_text, theme.size.font_size_md, theme.size.line_height_md, theme.color.text_4, {
					id = "go_live_empty",
				}),
			}
		),
		ui.each(shown, function(source: state.ShareSource)
			return source_tile { source = source, width = width }
		end, {
			key = function(source: state.ShareSource)
				return source.id
			end,
			direction = "row",
			flex_wrap = true,
			gap = theme.size.space_16,
			w_full = true,
		}),
	}
end)

local function source_list(tab: any)
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		segments("go_live_tab", TABS, function(key: any): boolean
			return tab:get() == key
		end, function(key: any)
			tab:set(key)
		end, true),
		source_grid { tab = tab },
	}
end

local function size_text(): string
	local now = preview()
	if not now.running then
		return "Starting preview..."
	end
	return string.format("%dx%d at %d FPS", now.width, now.height, now.fps)
end

local function preview_box()
	return ui.div {
		id = "go_live_preview",
		relative = true,
		w_full = true,
		h = preview_height,
		rounded = theme.size.radius_md,
		overflow = "hidden",
		bg = theme.color.bg_1,
		ui.image {
			id = "go_live_preview_image",
			src = "host:screen/preview",
			absolute = true,
			left = 0,
			top = 0,
			w_full = true,
			h_full = true,
			fit = "contain",
		},
		ui.row {
			absolute = true,
			left = n(theme.size.space_8),
			bottom = n(theme.size.space_8),
			max_w = function()
				return content_width() - n(theme.size.space_16)
			end,
			items = "center",
			gap = theme.size.space_4,
			px = theme.size.space_8,
			py = theme.size.space_2,
			rounded = theme.size.radius_sm,
			bg = theme.color.overlay_button_background,
			icon { name = "screen_share", size = n(theme.size.icon_size_sm), color = theme.color.text_2 },
			ui.text {
				function()
					return preview().name
				end,
				id = "go_live_preview_name",
				literal = true,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				weight = theme.weight.medium,
				color = theme.color.text_1,
			},
		},
	}
end

local function quality()
	local function choose(resolution: string, fps: number)
		local now = preview()
		actions.preview_screen(now.id, resolution, fps)
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		kit.caption("Stream Quality"),
		ui.row {
			w_full = true,
			gap = theme.size.space_16,
			ui.column {
				grow = 1,
				basis = 0,
				gap = theme.size.space_8,
				kit.text("Resolution", theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_3),
				segments("go_live_resolution", RESOLUTIONS, function(key: any): boolean
					return preview().resolution == key
				end, function(key: any)
					choose(key, preview().fps)
				end, true),
			},
			ui.column {
				grow = 1,
				basis = 0,
				gap = theme.size.space_8,
				kit.text("Frame Rate", theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_3),
				segments("go_live_fps", FRAME_RATES, function(key: any): boolean
					return preview().fps == key
				end, function(key: any)
					choose(preview().resolution, key)
				end, false),
			},
		},
	}
end

local function picked()
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		preview_box(),
		kit.text(size_text, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, {
			id = "go_live_preview_size",
		}),
		quality(),
	}
end

local function go_live_button()
	return ui.button {
		id = "go_live_start",
		h = theme.size.button_height_md,
		px = theme.size.space_16,
		shrink = 0,
		rounded = theme.size.radius_sm,
		border = theme.size.border_thickness,
		border_color = theme.color.button_border,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		justify = "center",
		items = "center",
		opacity = 0.5,
		cursor = "not-allowed",
		bg = theme.color.accent_3,
		tooltip = "Going live comes in a later version",
		ui.text {
			"Go Live",
			literal = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = theme.color.text_0,
		},
	}
end

local function choosing(): boolean
	return preview().id ~= ""
end

return ui.component(function()
	local open = ui.use_state(false)
	local tab = ui.use_state("window")
	ui.effect(function()
		local wanted = sources().open
		if open:peek() ~= wanted then
			if wanted then
				tab:set("window")
			end
			open:set(wanted)
		end
	end)
	local was_open = false
	ui.effect(function()
		local now = open:get()
		if was_open and not now and state.voice.screens:peek().open then
			actions.set_go_live(false)
		end
		was_open = now
	end)
	local function close()
		open:set(false)
	end
	local steps = ui.computed(function(): { string }
		return { if choosing() then "picked" else "list" }
	end)
	return dialog.popup {
		id = "go_live_picker",
		label = "Screen Share",
		open = open,
		width = theme.size.modal_width_lg,
		content = ui.show(function()
			return open:get()
		end, ui.column {
			w_full = true,
			min_h = 0,
			dialog.popup_header("go_live_close", "Screen Share", close),
			ui.each(steps, function(step: string)
				return ui.column {
					w_full = true,
					p = theme.size.space_16,
					if step == "picked" then picked() else source_list(tab),
				}
			end, {
				key = function(step: string)
					return step
				end,
				w_full = true,
				shrink = 1,
				min_h = 0,
			}),
			dialog.popup_footer(
				ui.show(
					choosing,
					kit.button {
						id = "go_live_change",
						text = "Change",
						height = theme.size.button_height_md,
						on_click = actions.stop_screen_preview,
					}
				),
				kit.button {
					id = "go_live_cancel",
					text = "Cancel",
					height = theme.size.button_height_md,
					on_click = close,
				},
				ui.show(choosing, go_live_button())
			),
		}),
	}
end)
