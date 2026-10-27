local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local avatar = require("@components/avatar")

local avatar_lg = theme.number(theme.size.avatar_size_lg)
local icon_md = theme.number(theme.size.icon_size_md)
local mutual_size = 20

local function themed(profile: state.DmProfile): boolean
	return profile.theme_top ~= "" and profile.theme_bottom ~= ""
end

local function surface(profile: state.DmProfile): string
	return if themed(profile) then profile.theme_top else theme.color.bg_4
end

local function label(text: string)
	return ui.text {
		text,
		text_size = theme.size.font_size_xs,
		line_height = theme.size.line_height_xs,
		weight = theme.weight.bold,
		color = theme.color.text_2,
	}
end

local function body(text: string)
	return ui.text {
		text,
		literal = true,
		w_full = true,
		wrap = true,
		text_size = theme.size.font_size_sm,
		line_height = theme.size.line_height_sm,
		color = theme.color.text_3,
	}
end

local function round_button(id: string, name: string, tip: string, dimmed: boolean, press: (any) -> ())
	return ui.button {
		id = id,
		w = theme.size.button_size,
		h = theme.size.button_size,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.button_size,
		justify = "center",
		items = "center",
		bg = theme.color.overlay_button_background,
		hover = { bg = theme.color.bg_1 },
		opacity = if dimmed then theme.size.disabled_opacity_control else 1,
		cursor = "pointer",
		tooltip = tip,
		on_press = press,
		icon { name = name, size = icon_md(), color = theme.color.text_1 },
	}
end

local function banner_buttons(profile: state.DmProfile)
	local friend = nil
	if profile.friend == "add" or profile.friend == "accept" then
		local tip = if profile.friend == "add" then "Add Friend" else "Accept Friend Request"
		friend = round_button("dm_profile_friend", "user_plus", tip, false, function()
			actions.add_dm_profile_friend()
		end)
	elseif profile.friend == "sent" then
		friend = round_button("dm_profile_friend", "user_plus", "Friend Request Sent", true, function() end)
	end
	return ui.row {
		absolute = true,
		top = theme.number(theme.size.space_12)(),
		right = theme.number(theme.size.space_12)(),
		gap = theme.size.space_8,
		friend,
		round_button("dm_profile_more", "more", "More", false, function(point: any)
			actions.open_dm_profile_menu(profile.id, point.x, point.y)
		end),
	}
end

local function banner(profile: state.DmProfile)
	local glow = nil
	if profile.effect and themed(profile) then
		glow = ui.div {
			id = "dm_profile_effect",
			absolute = true,
			left = 0,
			top = 0,
			w_full = true,
			h = theme.size.dm_profile_banner_height,
			pointer_events = "none",
			gradient = { profile.theme_bottom .. "66", "#00000000" },
		}
	end
	return ui.div {
		relative = true,
		w_full = true,
		h = theme.size.dm_profile_banner_height,
		shrink = 0,
		overflow = "hidden",
		bg = if profile.accent ~= "" then profile.accent else theme.color.bg_2,
		ui.show(
			profile.banner ~= "",
			ui.image { src = profile.banner, w_full = true, h = theme.size.dm_profile_banner_height, fit = "cover" }
		),
		glow,
		banner_buttons(profile),
	}
end

local function portrait(profile: state.DmProfile)
	return ui.row {
		absolute = true,
		left = theme.number(theme.size.space_16)(),
		top = theme.number(theme.size.dm_profile_avatar_top)(),
		w = theme.size.dm_profile_avatar_ring,
		h = theme.size.dm_profile_avatar_ring,
		justify = "center",
		items = "center",
		rounded = theme.size.dm_profile_avatar_ring,
		bg = surface(profile),
		avatar { src = profile.avatar, size = avatar_lg(), status = profile.status, cutout = surface(profile) },
	}
end

local function tag(profile: state.DmProfile)
	if profile.tag == "" then
		return nil
	end
	return ui.row {
		id = "dm_profile_tag",
		shrink = 0,
		items = "center",
		gap = theme.size.space_2,
		h = theme.size.tag_height,
		px = theme.size.space_4,
		rounded = theme.size.radius_xs,
		bg = theme.color.bg_2,
		ui.show(
			profile.tag_badge ~= "",
			ui.image { src = profile.tag_badge, w = theme.size.icon_size_xs, h = theme.size.icon_size_xs, fit = "contain" }
		),
		ui.text {
			profile.tag,
			literal = true,
			text_size = theme.size.font_size_xs,
			line_height = theme.size.tag_height,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
	}
end

local function names(profile: state.DmProfile)
	return ui.column {
		w_full = true,
		ui.text {
			profile.name,
			literal = true,
			w_full = true,
			truncate = true,
			text_size = theme.size.font_size_lg,
			line_height = theme.size.line_height_lg,
			weight = theme.weight.bold,
			color = theme.color.accent_1,
		},
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_6,
			ui.text {
				profile.username,
				literal = true,
				shrink = 1,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				color = theme.color.text_3,
			},
			tag(profile),
		},
	}
end

local function stacked(icons: { state.DmMutual }, profile: state.DmProfile)
	local children = {}
	for index, mutual in icons do
		table.insert(
			children,
			ui.row {
				key = mutual.key,
				w = mutual_size,
				h = mutual_size,
				shrink = 0,
				ml = if index > 1 then -4 else 0,
				rounded = mutual_size,
				overflow = "hidden",
				justify = "center",
				items = "center",
				border = 2,
				border_color = surface(profile),
				bg = theme.color.bg_2,
				if mutual.src ~= ""
					then ui.image { src = mutual.src, w = mutual_size, h = mutual_size, fit = "cover" }
					else ui.text {
						mutual.initial,
						literal = true,
						text_size = theme.size.font_size_tag,
						weight = theme.weight.semibold,
						color = theme.color.text_2,
					},
			}
		)
	end
	return ui.row { items = "center", table.unpack(children) }
end

local function mutual_text(text: string)
	return ui.text {
		text,
		literal = true,
		text_size = theme.size.font_size_sm,
		line_height = theme.size.line_height_sm,
		color = theme.color.text_4,
	}
end

local function mutuals(profile: state.DmProfile)
	if profile.servers == "" and profile.friends == "" then
		return nil
	end
	local function open()
		actions.open_full_profile(profile.id)
	end
	local servers = if profile.servers ~= ""
		then ui.row {
			id = "dm_profile_servers",
			items = "center",
			gap = theme.size.space_6,
			cursor = "pointer",
			on_click = open,
			stacked(profile.server_icons, profile),
			mutual_text(profile.servers),
		}
		else nil
	local friends = if profile.friends ~= ""
		then ui.row {
			id = "dm_profile_friends",
			items = "center",
			gap = theme.size.space_6,
			cursor = "pointer",
			on_click = open,
			if profile.servers ~= "" then mutual_text("\u{2022}") else nil,
			stacked(profile.friend_icons, profile),
			mutual_text(profile.friends),
		}
		else nil
	return ui.row {
		w_full = true,
		items = "center",
		flex_wrap = true,
		gap = theme.size.space_6,
		servers,
		friends,
	}
end

local function loading()
	local function bar(width: number)
		return ui.div { w = width, h = 10, rounded = 5, bg = theme.color.interactive_muted }
	end
	return ui.column {
		id = "dm_profile_loading",
		w_full = true,
		gap = theme.size.space_8,
		opacity = 0.6,
		bar(200),
		bar(140),
	}
end

local function build(profile: state.DmProfile)
	if not profile.open then
		return ui.div {}
	end
	return ui.column {
		relative = true,
		w_full = true,
		h_full = true,
		gradient = if themed(profile) then { profile.theme_top, profile.theme_bottom } else nil,
		banner(profile),
		ui.column {
			w_full = true,
			grow = 1,
			min_h = 0,
			overflow = "hidden",
			pt = theme.size.dm_profile_content_top,
			px = theme.size.space_16,
			pb = theme.size.space_16,
			gap = theme.size.space_16,
			names(profile),
			mutuals(profile),
			if profile.loading then loading() elseif profile.bio ~= "" then body(profile.bio) else nil,
			ui.column { id = "dm_profile_since", w_full = true, gap = theme.size.space_4, label("Member Since"), body(profile.since) },
			ui.div { grow = 1 },
			ui.button {
				id = "dm_profile_full",
				w_full = true,
				h = theme.size.button_height_md,
				shrink = 0,
				border = 0,
				focus = { border = 0 },
				rounded = theme.size.radius_sm,
				justify = "center",
				items = "center",
				bg = theme.color.bg_2,
				hover = { bg = theme.color.bg_1 },
				cursor = "pointer",
				on_click = function()
					actions.open_full_profile(profile.id)
				end,
				ui.text {
					"View Full Profile",
					text_size = theme.size.font_size_sm,
					line_height = theme.size.line_height_sm,
					weight = theme.weight.semibold,
					color = theme.color.text_2,
				},
			},
		},
		portrait(profile),
	}
end

return function()
	return ui.column {
		w = theme.size.dm_profile_width,
		h_full = true,
		ui.presence(function()
			return state.dms.profile:get()
		end, build),
	}
end
