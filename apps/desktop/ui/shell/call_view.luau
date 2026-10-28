local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")

local TILE_RATIO = 9 / 16
local AVATAR_SHARE = 0.5
local VOLUME_MAX = 200
local VOLUME_STEP = 1

local gap = theme.number(theme.size.space_8)
local stage_padding = theme.number(theme.size.space_16)
local avatar_size = theme.number(theme.size.call_tile_avatar)
local min_tile = theme.number(theme.size.call_tile_min_width)
local control_size = theme.number(theme.size.call_control_size)
local badge_icon = theme.number(theme.size.voice_badge_icon)
local grabber = theme.number(theme.size.slider_grabber_size)
local bar = theme.number(theme.size.slider_track_height)

local menu_style = {
	w = theme.size.call_menu_width,
	px = theme.size.space_8,
	py = theme.size.space_6,
	gap = theme.size.space_2,
	rounded = theme.size.radius_sm,
	bg = theme.color.bg_3,
	border = theme.size.border_thickness,
	border_color = theme.color.border,
}

local function call(): state.VoiceCall
	return state.voice.call:get()
end

local function controls(): state.VoiceControls
	return state.voice.controls:get()
end

local function speaking(id: string): boolean
	return state.voice.speaking:get()[id] == true
end

local function tile_width(count: number, width: number, height: number): number
	if count <= 0 or width <= 0 or height <= 0 then
		return min_tile()
	end
	local spacing = gap()
	local best = 0
	for columns = 1, count do
		local rows = math.ceil(count / columns)
		local across = (width - spacing * (columns - 1)) / columns
		local down = (height - spacing * (rows - 1)) / rows / TILE_RATIO
		best = math.max(best, math.min(across, down))
	end
	return math.max(math.floor(best), min_tile())
end

local function header()
	return ui.row {
		w_full = true,
		h = theme.size.channel_header_height,
		shrink = 0,
		items = "center",
		gap = theme.size.space_8,
		pl = theme.size.space_16,
		icon { name = "volume", size = theme.number(theme.size.icon_size_md)(), color = theme.color.text_4 },
		ui.text {
			function()
				return call().name
			end,
			id = "call_name",
			literal = true,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_md,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
		ui.div { grow = 1 },
		icon_button {
			id = "call_open_chat",
			name = "chat",
			tooltip = "Open Chat",
			hit_size = theme.number(theme.size.button_size)(),
			filled_hover = true,
			on_click = actions.open_call_chat(),
		},
		ui.div { w = theme.size.window_controls_width, h_full = true, shrink = 0 },
	}
end

local function state_icons(tile: state.CallTile): { any }
	local icons = {}
	if tile.local_muted then
		table.insert(icons, icon { name = "volume_off", size = badge_icon(), color = theme.color.red_3 })
	end
	if tile.deafened then
		table.insert(icons, icon { name = "headphones_off", size = badge_icon(), color = theme.color.red_3 })
	elseif tile.muted then
		table.insert(icons, icon { name = "mic_off", size = badge_icon(), color = theme.color.red_3 })
	end
	return icons
end

local volume_slider = ui.component(function(props: { tile: state.CallTile })
	local track = ui.ref()
	local dragging = ui.use_state(false)
	local volume = ui.use_state(props.tile.volume)
	local function fraction(): number
		return math.clamp(volume:get() / VOLUME_MAX, 0, 1)
	end
	local function pick(x: number)
		local width = track.width or 0
		if width <= 0 then
			return
		end
		local raw = math.clamp(x / width, 0, 1) * VOLUME_MAX
		volume:set(math.floor(raw / VOLUME_STEP + 0.5) * VOLUME_STEP)
	end
	local function commit()
		actions.set_user_volume(props.tile.id, volume:get())
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_6,
		px = theme.size.space_8,
		py = theme.size.space_6,
		ui.row {
			w_full = true,
			justify = "between",
			ui.text {
				"User Volume",
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				weight = theme.weight.medium,
				color = theme.color.text_3,
			},
			ui.text {
				function()
					return string.format("%d%%", volume:get())
				end,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				color = theme.color.text_4,
			},
		},
		ui.div {
			id = "call_menu_volume",
			ref = track,
			relative = true,
			w_full = true,
			h = theme.size.slider_grabber_size,
			cursor = "pointer",
			label = "User Volume",
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
				if dragging:get() then
					dragging:set(false)
					commit()
				end
			end,
			on_hover = function(inside: boolean)
				if not inside and dragging:get() then
					dragging:set(false)
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
	}
end)

local function participant_menu(tile: state.CallTile): { any }
	return {
		volume_slider { tile = tile },
		ui.div { h = theme.size.border_thickness, my = theme.size.space_4, mx = theme.size.space_4, bg = theme.color.border },
		ui.menu_item {
			"Mute",
			id = "call_menu_mute",
			checked = tile.local_muted,
			on_click = function()
				actions.set_local_mute(tile.id, not tile.local_muted)
			end,
		},
	}
end

local function tile_view(tile: state.CallTile, size: () -> number, focused: any)
	local function lit(): boolean
		return speaking(tile.id)
	end
	local function face(): number
		return math.min(avatar_size(), size() * TILE_RATIO * AVATAR_SHARE)
	end
	return ui.div {
		id = "call_tile_" .. tile.id,
		relative = true,
		shrink = 0,
		w = size,
		h = function()
			return size() * TILE_RATIO
		end,
		rounded = theme.size.radius_md,
		overflow = "hidden",
		bg = theme.color.bg_2,
		border = theme.size.call_ring_width,
		border_color = function()
			return if lit() then theme.color.speaking_ring else "#00000000"
		end,
		justify = "center",
		items = "center",
		cursor = "pointer",
		on_click = function()
			focused:set(if focused:get() == tile.id then "" else tile.id)
		end,
		context_menu_style = menu_style,
		context_menu = if tile.me
			then nil
			else function()
				return participant_menu(tile)
			end,
		ui.div {
			id = "call_avatar_" .. tile.id,
			w = face,
			h = face,
			rounded = face,
			overflow = "hidden",
			bg = theme.color.bg_3,
			ui.show(
				tile.avatar ~= "",
				ui.image {
					src = tile.avatar,
					w = face,
					h = face,
					fit = "cover",
				}
			),
		},
		ui.show(
			tile.video == "camera",
			ui.image {
				id = "call_video_" .. tile.id,
				src = "host:video/" .. tile.id,
				absolute = true,
				left = 0,
				top = 0,
				w = size,
				h = function()
					return size() * TILE_RATIO
				end,
				fit = "cover",
			}
		),
		ui.show(
			function()
				return tile.video == "camera" and not state.voice.video_available:get()
			end,
			ui.text {
				"Video unavailable",
				id = "call_video_unavailable_" .. tile.id,
				absolute = true,
				top = gap(),
				left = gap(),
				px = theme.size.space_8,
				py = theme.size.space_2,
				rounded = theme.size.radius_sm,
				bg = theme.color.overlay_button_background,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				color = theme.color.text_2,
			}
		),
		ui.row {
			absolute = true,
			left = gap(),
			bottom = gap(),
			max_w = function()
				return size() - gap() * 2
			end,
			items = "center",
			gap = theme.size.space_4,
			px = theme.size.space_8,
			py = theme.size.space_2,
			rounded = theme.size.radius_sm,
			bg = theme.color.overlay_button_background,
			ui.text {
				tile.name,
				literal = true,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				weight = theme.weight.medium,
				color = theme.color.text_1,
			},
			table.unpack(state_icons(tile)),
		},
	}
end

local stage = ui.component(function()
	local area = ui.ref()
	local focused = ui.use_state("")
	local function shown(): { state.CallTile }
		local tiles = call().tiles
		local focus = focused:get()
		if focus == "" then
			return tiles
		end
		for _, tile in tiles do
			if tile.id == focus then
				return { tile }
			end
		end
		return tiles
	end
	local tiles = ui.computed(shown)
	local function size(): number
		local inset = stage_padding() * 2
		return tile_width(#tiles:get(), (area.width or 0) - inset, (area.height or 0) - inset)
	end
	local reported = ""
	ui.effect(function()
		local width = size()
		local focus = focused:get()
		local views = {}
		local keys = {}
		for _, tile in tiles:get() do
			if tile.video == "camera" then
				table.insert(views, {
					id = tile.id,
					w = width,
					h = width * TILE_RATIO,
					focused = focus == tile.id,
				})
				table.insert(keys, string.format("%s:%d:%s", tile.id, width, tostring(focus == tile.id)))
			end
		end
		local key = table.concat(keys, ",")
		if key ~= reported then
			reported = key
			actions.set_video_views(views)
		end
	end)
	return ui.div {
		ref = area,
		id = "call_stage",
		w_full = true,
		grow = 1,
		min_h = 0,
		p = theme.size.space_16,
		justify = "center",
		items = "center",
		overflow = "hidden",
		ui.each(tiles, function(tile: state.CallTile)
			return tile_view(tile, size, focused)
		end, {
			key = function(tile: state.CallTile)
				return tile.key
			end,
			direction = "row",
			flex_wrap = true,
			justify = "center",
			items = "center",
			gap = theme.size.space_8,
			w_full = true,
		}),
	}
end)

local control = ui.component(function(props: {
	id: string,
	name: string | () -> string,
	tooltip: string | () -> string,
	on_click: any?,
	red: (() -> boolean)?,
	disabled: boolean?,
})
	local hovered = ui.use_state(false)
	local disabled = props.disabled == true
	local function red(): boolean
		return props.red ~= nil and props.red()
	end
	return ui.button {
		id = props.id,
		w = theme.size.call_control_size,
		h = theme.size.call_control_size,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.call_control_size,
		justify = "center",
		items = "center",
		opacity = if disabled then 0.5 else 1,
		cursor = if disabled then "not-allowed" else "pointer",
		bg = function()
			local lifted = hovered:get() and not disabled
			if red() then
				return if lifted then theme.color.red_4 else theme.color.red_3
			end
			return if lifted then theme.color.bg_1 else theme.color.bg_2
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		tooltip = if type(props.tooltip) == "function" then ui.text { props.tooltip } else props.tooltip,
		on_click = if disabled then nil else props.on_click,
		icon {
			name = props.name,
			size = theme.number(theme.size.icon_size_lg)(),
			color = function()
				return if red() then theme.color.text_0 else theme.color.text_2
			end,
		},
	}
end)

local function muted(): boolean
	local now = controls()
	return now.muted or now.server_muted
end

local function deafened(): boolean
	local now = controls()
	return now.deafened or now.server_deafened
end

local function bottom_bar()
	return ui.row {
		w_full = true,
		shrink = 0,
		justify = "center",
		items = "center",
		gap = theme.size.space_12,
		pb = theme.size.space_24,
		pt = theme.size.space_8,
		control { id = "call_camera", name = "video", tooltip = "Turn On Camera", disabled = true },
		control { id = "call_screen", name = "screen_share", tooltip = "Share Your Screen", disabled = true },
		control {
			id = "call_mute",
			name = function()
				return if muted() then "mic_off" else "mic"
			end,
			tooltip = function()
				if controls().server_muted then
					return "Server Muted"
				end
				return if muted() then "Unmute" else "Mute"
			end,
			red = muted,
			on_click = actions.toggle_mute(),
		},
		control {
			id = "call_deafen",
			name = function()
				return if deafened() then "headphones_off" else "headphones"
			end,
			tooltip = function()
				if controls().server_deafened then
					return "Server Deafened"
				end
				return if deafened() then "Undeafen" else "Deafen"
			end,
			red = deafened,
			on_click = actions.toggle_deafen(),
		},
		control {
			id = "call_disconnect",
			name = "phone_off",
			tooltip = "Disconnect",
			red = function()
				return true
			end,
			on_click = actions.leave_voice(),
		},
	}
end

return function()
	return ui.column {
		id = "call_view",
		w_full = true,
		h_full = true,
		bg = theme.color.bg_4,
		header(),
		stage {},
		bottom_bar(),
	}
end
