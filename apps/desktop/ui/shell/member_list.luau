local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")

local AVATAR = 32
local LEADING = 1.618
local NAME_LINE = 20
local ACTIVITY_LINE = 14
local TAG_TEXT = 10
local TAG_LINE = 15
local TAG_SIDES = 5
local SEARCH_HEIGHT = 24
local SEARCH_DELAY = 350
local SEARCH_FILL = 0.3
local PLACEHOLDER_NAME = 12
local PLACEHOLDER_WIDTHS = { 96, 72, 120, 84, 108, 64, 100, 80 }

local offset = ui.state("members.offset", 0)
local viewport = ui.state("members.viewport", 0)
local query = ui.state("members.query", "")

local member_height = theme.number(theme.size.member_row_height)
local font_tiny = theme.number(theme.size.font_tiny)
local space_4 = theme.number(theme.size.space_4)
local space_16 = theme.number(theme.size.space_16)
local space_24 = theme.number(theme.size.space_24)

local function group_height(): number
	return space_24() + font_tiny() * LEADING + space_4()
end

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

local function layer(props: { [string]: any })
	props.absolute = true
	props.left = 0
	props.top = 0
	props.right = 0
	props.bottom = 0
	props.pointer_events = "none"
	return ui.div(props)
end

local function header(row: any)
	return ui.div {
		w_full = true,
		px = theme.size.space_16,
		pt = theme.size.space_24,
		pb = theme.size.space_4,
		ui.text {
			row.title,
			literal = true,
			truncate = true,
			text_size = theme.size.font_tiny,
			line_height = function()
				return font_tiny() * LEADING
			end,
			weight = theme.weight.semibold,
			color = theme.color.header_secondary,
		},
	}
end

local function tag(label: string, bg: string, color: string, size: any, line: number?, sides: any, weight: number?)
	return ui.div {
		shrink = 0,
		px = sides,
		rounded = theme.size.radius_sm,
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

local function name_line(row: any)
	local offline = row.status == "offline"
	local color = if row.color ~= ""
		then row.color
		elseif offline then theme.color.text_muted
		else theme.color.text_normal
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
			text_size = theme.size.font_md,
			line_height = NAME_LINE,
			weight = theme.weight.semibold,
			color = color,
		},
		if extra
			then ui.text {
				row.extra,
				literal = true,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_md,
				line_height = NAME_LINE,
				weight = theme.weight.medium,
				color = theme.color.text_muted,
			}
			else nil,
		if row.bot
			then tag("BOT", theme.color.brand, theme.color.text_on_brand, TAG_TEXT, TAG_LINE, TAG_SIDES, theme.weight.bold)
			else nil,
		if row.badge ~= ""
			then tag(
				row.badge,
				theme.color.mention_background,
				theme.color.mention_text,
				theme.size.font_tiny,
				nil,
				theme.size.space_4,
				nil
			)
			else nil,
	}
end

local function member(row: any)
	return ui.row {
		h = theme.size.member_row_height,
		mx = theme.size.space_8,
		px = theme.size.space_8,
		gap = theme.size.space_12,
		items = "center",
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		opacity = if row.status == "offline" then 0.5 else 1,
		hover = { bg = theme.color.modifier_hover },
		on_press = function(point: any)
			actions.open_member_profile(row.id, point.x, point.y)
		end,
		context_menu = function(point: any)
			actions.member_menu(row.id, point.x, point.y)
			return nil
		end,
		avatar { src = row.avatar, size = AVATAR, status = row.status, cutout = theme.color.background_secondary },
		ui.column {
			min_w = 0,
			name_line(row),
			if row.activity ~= ""
				then ui.text {
					row.activity,
					literal = true,
					truncate = true,
					text_size = theme.size.font_tiny,
					line_height = ACTIVITY_LINE,
					color = theme.color.text_muted,
				}
				else nil,
		},
	}
end

local function placeholder(row: any)
	local width = PLACEHOLDER_WIDTHS[row.index % #PLACEHOLDER_WIDTHS + 1]
	return ui.row {
		h = theme.size.member_row_height,
		mx = theme.size.space_8,
		px = theme.size.space_8,
		gap = theme.size.space_12,
		items = "center",
		ui.div { w = AVATAR, h = AVATAR, shrink = 0, rounded = AVATAR, bg = theme.color.modifier_active },
		ui.div {
			w = width,
			h = PLACEHOLDER_NAME,
			rounded = PLACEHOLDER_NAME,
			bg = theme.color.modifier_active,
		},
	}
end

local function build(row: any)
	if row.kind == "group" then
		return header(row)
	elseif row.kind == "member" then
		return member(row)
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

local function search()
	local cancel: (() -> ())? = nil
	ui.effect(function()
		local text = query:get()
		actions.member_search(text)
		if cancel then
			cancel()
		end
		cancel = ui.after(SEARCH_DELAY, function()
			actions.member_search_remote(text)
		end)
	end)
	return ui.div {
		shrink = 0,
		px = theme.size.space_8,
		pt = theme.size.space_8,
		ui.row {
			relative = true,
			w_full = true,
			h = SEARCH_HEIGHT,
			layer { rounded = theme.size.radius_sm, bg = theme.color.interactive_muted, opacity = SEARCH_FILL },
			layer { rounded = theme.size.radius_sm, border = 1, border_color = theme.color.interactive_muted },
			ui.input {
				w_full = true,
				h = SEARCH_HEIGHT,
				px = theme.size.space_8,
				bg = "#00000000",
				border = 0,
				focus = { border = 0 },
				text_size = theme.size.font_sm,
				color = theme.color.text_normal,
				placeholder = "Search members",
				placeholder_color = theme.color.text_muted,
				value = query,
			},
		},
	}
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
		w = theme.size.member_list_width,
		h_full = true,
		shrink = 0,
		bg = theme.color.background_secondary,
		search(),
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
