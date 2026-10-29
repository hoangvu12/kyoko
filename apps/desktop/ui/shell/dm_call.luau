local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local call_view = require("./call_view")

local default_height = theme.number(theme.size.dm_call_height)
local min_height = theme.number(theme.size.dm_call_min_height)
local face_size = theme.number(theme.size.call_tile_avatar)
local CHAT_ROOM = 160

local function area(): state.DmCall
	return state.voice.dm_call:get()
end

local function person(entry: state.CallPerson)
	return ui.column {
		id = "dm_call_person_" .. entry.id,
		items = "center",
		gap = theme.size.space_8,
		opacity = if entry.ringing then 0.5 else 1,
		ui.div {
			w = face_size,
			h = face_size,
			rounded = face_size,
			overflow = "hidden",
			bg = theme.color.bg_3,
			ui.show(
				entry.avatar ~= "",
				ui.image {
					src = entry.avatar,
					w = face_size,
					h = face_size,
					fit = "cover",
				}
			),
		},
		ui.text {
			entry.name,
			literal = true,
			max_w = theme.size.call_tile_min_width,
			truncate = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.medium,
			color = theme.color.text_3,
		},
	}
end

local function join_button(props: { id: string, label: string, name: string, video: boolean })
	return ui.button {
		id = props.id,
		h = theme.size.button_size,
		px = theme.size.space_16,
		gap = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		border = 0,
		bg = if props.video then theme.color.bg_2 else theme.color.green_3,
		hover = { bg = if props.video then theme.color.bg_1 else theme.color.green_4 },
		color = if props.video then theme.color.text_2 else theme.color.text_0,
		cursor = "pointer",
		on_click = function()
			actions.start_call(area().channel_id, props.video)
		end,
		icon { name = props.name, size = theme.number(theme.size.icon_size_md)() },
		ui.text {
			props.label,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.semibold,
		},
	}
end

local function lobby()
	local people = ui.computed(function()
		return area().people
	end)
	return ui.column {
		id = "dm_call_lobby",
		w_full = true,
		grow = 1,
		min_h = 0,
		justify = "center",
		items = "center",
		gap = theme.size.space_24,
		ui.each(people, person, {
			key = function(entry: state.CallPerson)
				return entry.id
			end,
			direction = "row",
			justify = "center",
			gap = theme.size.space_24,
		}),
		ui.row {
			gap = theme.size.space_8,
			join_button { id = "dm_call_join", label = "Join Call", name = "phone", video = false },
			join_button { id = "dm_call_join_video", label = "Join with Video", name = "video", video = true },
		},
	}
end

return ui.component(function(props: { room: () -> number })
	local height = ui.use_state(default_height())
	local dragging = ui.use_state(false)
	local from = 0
	local start = 0
	local function clamp(value: number): number
		local most = math.max(min_height(), props.room() - CHAT_ROOM)
		return math.clamp(value, min_height(), most)
	end
	local function stop()
		dragging:set(false)
	end
	return ui.show(
		function()
			return area().shown
		end,
		ui.column {
			id = "dm_call",
			w_full = true,
			shrink = 0,
			h = function()
				return clamp(height:get())
			end,
			bg = theme.color.bg_3,
			ui.show(function()
				return area().joined and state.voice.call:get().channel_id == area().channel_id
			end, call_view { embedded = true }, lobby()),
			ui.div {
				id = "dm_call_resize",
				w_full = true,
				h = theme.size.dm_call_handle,
				shrink = 0,
				cursor = "row_resize",
				label = "Resize call",
				bg = function()
					return if dragging:get() then theme.color.accent_2 else theme.color.border
				end,
				on_press = function(event: any)
					dragging:set(true)
					from = event.y
					start = clamp(height:get())
				end,
				on_pointer_move = function(event: any)
					if dragging:get() then
						height:set(clamp(start + event.y - from))
					end
				end,
				on_release = stop,
			},
		}
	)
end)
