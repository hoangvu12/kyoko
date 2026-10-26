local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local avatar = require("@components/avatar")
local motion = require("@components/motion")

local avatar_sm = theme.number(theme.size.avatar_size_sm)
local icon_sm = theme.number(theme.size.icon_size_sm)

local function by_key(item: { key: string })
	return item.key
end

local function title(): string
	return `Members — {#state.dms.members:get()}`
end

local member = ui.component(function(props: { row: state.DmMember })
	local row = props.row
	local hovered = ui.use_state(false)
	return ui.div {
		w_full = true,
		py = theme.size.space_1,
		ui.row {
			id = `dm_member_{row.id}`,
			h = theme.size.member_row_height,
			ml = motion.slide(hovered, theme.size.space_8, theme.size.member_hover_shift),
			mr = theme.size.space_8,
			px = theme.size.space_8,
			gap = theme.size.space_12,
			items = "center",
			rounded = theme.size.radius_lg,
			cursor = "pointer",
			bg = function()
				return if hovered:get() then theme.color.hover else nil
			end,
			on_hover = function(inside: boolean)
				hovered:set(inside)
			end,
			on_press = function(point: any)
				actions.open_friend_profile(row.id, point.x, point.y)
			end,
			avatar { src = row.avatar, size = avatar_sm(), status = row.status, cutout = theme.color.bg_4 },
			ui.column {
				grow = 1,
				min_w = 0,
				ui.row {
					min_w = 0,
					gap = theme.size.space_6,
					items = "center",
					ui.text {
						row.name,
						literal = true,
						min_w = 0,
						truncate = true,
						text_size = theme.size.font_size_md,
						line_height = theme.size.line_height_md,
						weight = theme.weight.medium,
						color = theme.color.text_2,
					},
					if row.owner
						then ui.div {
							id = `dm_member_owner_{row.id}`,
							shrink = 0,
							tooltip = "Group Owner",
							icon { name = "crown", size = icon_sm(), color = theme.color.yellow_2 },
						}
						else nil,
				},
				if row.activity ~= ""
					then ui.text {
						row.activity,
						literal = true,
						truncate = true,
						text_size = theme.size.font_size_xs,
						line_height = theme.size.line_height_xs,
						color = theme.color.text_4,
					}
					else nil,
			},
		},
	}
end)

return function()
	return ui.column {
		w = theme.size.member_list_panel_width,
		h_full = true,
		ui.div {
			w_full = true,
			h = theme.size.group_header_height,
			shrink = 0,
			pt = theme.size.space_20,
			pr = theme.size.space_8,
			pb = theme.size.space_4,
			pl = theme.size.space_16,
			ui.text {
				title,
				truncate = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_xs,
				weight = theme.weight.medium,
				color = theme.color.text_4,
			},
		},
		ui.scroll {
			grow = 1,
			min_h = 0,
			w_full = true,
			ui.each(state.dms.members, function(row: state.DmMember)
				return member { row = row }
			end, { key = by_key, w_full = true }),
		},
	}
end
