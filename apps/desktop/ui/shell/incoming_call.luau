local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")

local avatar_size = theme.number(theme.size.incoming_call_avatar)
local ring = theme.number(theme.size.call_ring_width)

local function incoming(): state.IncomingCall
	return state.voice.incoming:get()
end

local answer = ui.component(function(props: {
	id: string,
	name: string,
	tooltip: string,
	tone: string,
	on_click: () -> (),
})
	local hovered = ui.use_state(false)
	local function fill(): string
		local lifted = hovered:get()
		if props.tone == "decline" then
			return if lifted then theme.color.red_4 else theme.color.red_3
		end
		if props.tone == "accept" then
			return if lifted then theme.color.green_4 else theme.color.green_3
		end
		return if lifted then theme.color.bg_1 else theme.color.bg_2
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
		cursor = "pointer",
		bg = fill,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		tooltip = props.tooltip,
		on_click = props.on_click,
		icon {
			name = props.name,
			size = theme.number(theme.size.icon_size_lg)(),
			color = if props.tone == "video" then theme.color.text_2 else theme.color.text_0,
		},
	}
end)

local function card()
	return ui.column {
		id = "incoming_call",
		w = theme.size.incoming_call_width,
		items = "center",
		gap = theme.size.space_8,
		px = theme.size.space_16,
		pt = theme.size.space_24,
		pb = theme.size.space_20,
		rounded = theme.size.radius_lg,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		ui.div {
			w = function()
				return avatar_size() + ring() * 4
			end,
			h = function()
				return avatar_size() + ring() * 4
			end,
			rounded = function()
				return avatar_size() + ring() * 4
			end,
			justify = "center",
			items = "center",
			border = theme.size.call_ring_width,
			border_color = theme.color.speaking_ring,
			ui.div {
				w = avatar_size,
				h = avatar_size,
				rounded = avatar_size,
				overflow = "hidden",
				bg = theme.color.bg_2,
				justify = "center",
				items = "center",
				ui.show(
					function()
						return incoming().avatar ~= ""
					end,
					ui.image {
						src = function()
							return incoming().avatar
						end,
						w = avatar_size,
						h = avatar_size,
						fit = "cover",
					},
					icon { name = "group", size = theme.number(theme.size.icon_size_lg)(), color = theme.color.text_3 }
				),
			},
		},
		ui.text {
			function()
				return incoming().name
			end,
			id = "incoming_call_name",
			literal = true,
			mt = theme.size.space_8,
			max_w = theme.size.incoming_call_width,
			truncate = true,
			text_size = theme.size.font_size_lg,
			line_height = theme.size.line_height_lg,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
		ui.text {
			function()
				return if incoming().group then "Incoming Group Call…" else "Incoming Call…"
			end,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			color = theme.color.text_4,
		},
		ui.row {
			mt = theme.size.space_16,
			gap = theme.size.space_16,
			items = "center",
			answer {
				id = "incoming_call_decline",
				name = "phone_off",
				tooltip = "Decline",
				tone = "decline",
				on_click = function()
					actions.decline_call(incoming().channel_id)
				end,
			},
			answer {
				id = "incoming_call_video",
				name = "video",
				tooltip = "Accept with Video",
				tone = "video",
				on_click = function()
					actions.accept_call(incoming().channel_id, true)
				end,
			},
			answer {
				id = "incoming_call_accept",
				name = "phone",
				tooltip = "Accept",
				tone = "accept",
				on_click = function()
					actions.accept_call(incoming().channel_id, false)
				end,
			},
		},
	}
end

return function()
	local open = ui.computed(function()
		return incoming().open
	end)
	return ui.layer {
		open = open,
		label = "Incoming call",
		placement = { x = "center", y = "start", inset = theme.number(theme.size.incoming_call_top)() },
		p = 0,
		border = 0,
		rounded = theme.size.radius_lg,
		bg = "#00000000",
		ui.show(function()
			return incoming().open
		end, card()),
	}
end
