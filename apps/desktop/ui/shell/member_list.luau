local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local motion = require("@components/motion")
local icon = require("@components/icon")

local PLACEHOLDER_NAME = 12
local PLACEHOLDER_WIDTHS = { 96, 72, 120, 84, 108, 64, 100, 80 }

local offset = ui.state("members.offset", 0)
local viewport = ui.state("members.viewport", 0)

local member_height = theme.number(theme.size.member_row_pitch)
local group_height = theme.number(theme.size.group_header_height)
local space_16 = theme.number(theme.size.space_16)
local avatar_sm = theme.number(theme.size.avatar_size_sm)
local icon_xs = theme.number(theme.size.icon_size_xs)
local icon_sm = theme.number(theme.size.icon_size_sm)

local function heights(): { number }
	local list = table.clone(state.members.heights:get())
	table.insert(list, space_16())
	return list
end

local function entry(i: number): any
	local window = state.members.window:get()
	local row = window.rows[i - window.first]
	if row then
		return row
	end
	if i > #state.members.heights:get() then
		return { kind = "spacer" }
	end
	return { kind = "placeholder", index = i - 1 }
end

local function header(row: any)
	return ui.div {
		w_full = true,
		h = theme.size.group_header_height,
		pt = theme.size.space_20,
		pr = theme.size.space_8,
		pb = theme.size.space_4,
		pl = theme.size.space_16,
		ui.text {
			row.title,
			literal = true,
			truncate = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_xs,
			weight = theme.weight.medium,
			color = theme.color.text_4,
		},
	}
end

local function tag(label: string, bg: string, color: string, size: any, line: any, sides: any, weight: number?)
	return ui.div {
		shrink = 0,
		px = sides,
		rounded = theme.size.radius_xs,
		bg = bg,
		ui.text {
			label,
			literal = true,
			text_size = size,
			line_height = line,
			weight = weight,
			color = color,
		},
	}
end

local function note(kind: string, id: string, text: string?, glyph: string, color: string)
	if text == nil or text == "" then
		return nil
	end
	return ui.div {
		id = `member_{kind}_{id}`,
		shrink = 0,
		tooltip = text,
		icon { name = glyph, size = icon_xs(), color = color },
	}
end

local function crown(row: any)
	if not row.crown then
		return nil
	end
	return ui.div {
		id = `member_crown_{row.id}`,
		shrink = 0,
		tooltip = "Server Owner",
		icon { name = "crown", size = icon_sm(), color = theme.color.yellow_2 },
	}
end

local function decorations(row: any)
	local list = row.decorations or {}
	if #list == 0 then
		return nil
	end
	local marks = {}
	for _, mark in list do
		if mark.src ~= "" then
			table.insert(
				marks,
				ui.div {
					id = `member_decoration_{mark.key}_{row.id}`,
					shrink = 0,
					tooltip = if mark.tooltip ~= "" then mark.tooltip else nil,
					ui.svg { src = mark.src, w = icon_sm(), h = icon_sm(), shrink = 0, tint = mark.color },
				}
			)
		end
	end
	return ui.row { shrink = 0, items = "center", gap = theme.size.space_2, table.unpack(marks) }
end

local function name_line(row: any)
	local offline = row.status == "offline"
	local color = if row.color ~= ""
		then row.color
		elseif offline then theme.color.text_4
		else theme.color.text_3
	local extra = row.extra ~= nil and row.extra ~= ""
	return ui.row {
		min_w = 0,
		items = "center",
		gap = theme.size.space_4,
		ui.text {
			row.name,
			literal = true,
			min_w = 0,
			shrink = if extra then 0 else 1,
			truncate = true,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_md,
			weight = theme.weight.medium,
			color = color,
		},
		if extra
			then ui.text {
				row.extra,
				literal = true,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_size_md,
				line_height = theme.size.line_height_md,
				weight = theme.weight.medium,
				color = theme.color.text_4,
			}
			else nil,
		if row.bot
			then tag(
				"BOT",
				theme.color.accent_2,
				theme.color.text_0,
				theme.size.font_size_tag,
				theme.size.bot_tag_height,
				theme.size.space_4,
				theme.weight.semibold
			)
			else nil,
		if row.badge ~= ""
			then tag(
				row.badge,
				theme.color.mention_background,
				theme.color.accent_1,
				theme.size.font_size_xs,
				nil,
				theme.size.space_4,
				nil
			)
			else nil,
		crown(row),
		decorations(row),
		note("timeout", row.id, row.timeout, "timer", theme.color.red_3),
		note("flags", row.id, row.notes, "shield", theme.color.yellow_3),
	}
end

local member = ui.component(function(props: { row: any })
	local row = props.row
	local hovered = ui.use_state(false)
	return ui.div {
		w_full = true,
		py = theme.size.space_1,
		opacity = if row.status == "offline" then theme.size.offline_opacity else 1,
		ui.row {
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
				if row.avatar_moving ~= nil and row.avatar_moving ~= "" then
					actions.member_avatar_hover(row.avatar_moving, inside)
				end
			end,
			on_press = function(point: any)
				actions.open_member_profile(row.id, point.x, point.y)
			end,
			context_menu = function(point: any)
				actions.member_menu(row.id, point.x, point.y)
				return nil
			end,
			avatar {
				src = row.avatar,
				size = avatar_sm(),
				status = row.status,
				cutout = theme.color.bg_4,
				moving = row.avatar_moving,
				playing = function()
					return hovered:get()
				end,
			},
			ui.column {
				min_w = 0,
				name_line(row),
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

local function placeholder(row: any)
	local width = PLACEHOLDER_WIDTHS[row.index % #PLACEHOLDER_WIDTHS + 1]
	return ui.div {
		w_full = true,
		py = theme.size.space_1,
		ui.row {
			h = theme.size.member_row_height,
			mx = theme.size.space_8,
			px = theme.size.space_8,
			gap = theme.size.space_12,
			items = "center",
			ui.div {
				w = theme.size.avatar_size_sm,
				h = theme.size.avatar_size_sm,
				shrink = 0,
				rounded = theme.size.avatar_size_sm,
				bg = theme.color.active_2,
			},
			ui.div {
				w = width,
				h = PLACEHOLDER_NAME,
				rounded = PLACEHOLDER_NAME,
				bg = theme.color.active_2,
			},
		},
	}
end

local function build(row: any)
	if row.kind == "group" then
		return header(row)
	elseif row.kind == "member" then
		return member { row = row }
	elseif row.kind == "placeholder" then
		return placeholder(row)
	end
	return ui.div {}
end
local function row(i: number)
	return ui.presence(function()
		return entry(i)
	end, build)
end

return function()
	ui.effect(function()
		state.timeline.channel_id:get()
		offset:set(0)
	end)
	ui.effect(function()
		local jump = state.members.jump:get()
		if jump.serial > 0 then
			offset:set(jump.offset)
		end
	end)
	ui.effect(function()
		actions.member_scroll(offset:get(), viewport:get(), member_height(), group_height())
	end)
	return ui.column {
		w_full = true,
		h_full = true,
		ui.list {
			label = "Members",
			grow = 1,
			min_h = 0,
			w_full = true,
			heights = heights,
			row = row,
			offset = offset,
			scrollbar = true,
			on_resize = function(size)
				viewport:set(size.h)
			end,
		},
	}
end
