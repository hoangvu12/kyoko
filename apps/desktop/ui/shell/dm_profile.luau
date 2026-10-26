local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local avatar = require("@components/avatar")

local avatar_lg = theme.number(theme.size.avatar_size_lg)
local icon_md = theme.number(theme.size.icon_size_md)

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
		mt = theme.size.space_4,
		text_size = theme.size.font_size_sm,
		line_height = theme.size.line_height_sm,
		color = theme.color.text_3,
	}
end

local function banner(profile: state.DmProfile)
	return ui.div {
		w_full = true,
		h = theme.size.dm_profile_banner_height,
		shrink = 0,
		overflow = "hidden",
		bg = if profile.accent ~= "" then profile.accent else theme.color.bg_2,
		ui.show(
			profile.banner ~= "",
			ui.image { src = profile.banner, w_full = true, h = theme.size.dm_profile_banner_height, fit = "cover" }
		),
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
		bg = theme.color.bg_4,
		avatar { src = profile.avatar, size = avatar_lg(), status = profile.status, cutout = theme.color.bg_4 },
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

local function about(profile: state.DmProfile)
	return ui.column {
		w_full = true,
		gap = theme.size.space_12,
		p = theme.size.space_12,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		if profile.bio ~= "" then ui.column { w_full = true, label("About Me"), body(profile.bio) } else nil,
		ui.column { w_full = true, label("Member Since"), body(profile.since) },
	}
end

local function mutual(id: string, text: string, user: string)
	if text == "" then
		return nil
	end
	return ui.row {
		id = id,
		w_full = true,
		p = theme.size.space_12,
		justify = "between",
		items = "center",
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		cursor = "pointer",
		hover = { bg = theme.color.active },
		on_click = function()
			actions.open_full_profile(user)
		end,
		ui.text {
			text,
			literal = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.semibold,
			color = theme.color.text_3,
		},
		icon { name = "chevron_right", size = icon_md(), color = theme.color.text_4 },
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
		banner(profile),
		ui.column {
			w_full = true,
			grow = 1,
			min_h = 0,
			overflow = "hidden",
			pt = theme.size.dm_profile_content_top,
			px = theme.size.space_16,
			pb = theme.size.space_16,
			gap = theme.size.space_12,
			names(profile),
			about(profile),
			mutual("dm_profile_servers", profile.servers, profile.id),
			mutual("dm_profile_friends", profile.friends, profile.id),
			ui.div { grow = 1 },
			ui.row {
				id = "dm_profile_full",
				w_full = true,
				justify = "center",
				cursor = "pointer",
				on_click = function()
					actions.open_full_profile(profile.id)
				end,
				ui.text {
					"View Full Profile",
					text_size = theme.size.font_size_sm,
					line_height = theme.size.line_height_sm,
					weight = theme.weight.semibold,
					color = theme.color.accent_1,
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
