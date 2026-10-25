local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")

local AVATAR = 32

return function()
	return ui.row {
		w_full = true,
		h = theme.size.user_panel_height,
		shrink = 0,
		px = theme.size.space_8,
		gap = theme.size.space_8,
		items = "center",
		bg = theme.color.user_panel_background,
		context_menu = function()
			return {
				ui.menu_item { "User Settings", on_click = function()
					actions.open_settings("my-account")
				end },
				ui.menu_item { "Log Out", on_click = actions.sign_out() },
			}
		end,
		ui.show(
			state.user.avatar_ready,
			ui.image {
				src = function()
					return state.user.avatar:get()
				end,
				w = AVATAR,
				h = AVATAR,
				on_hover = function(inside: boolean)
					actions.media_hover(if inside then string.sub(state.user.avatar:get(), 6) else "")
				end,
			}
		),
		ui.text {
			function()
				return state.session.status:get()
			end,
			literal = true,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_xs,
			color = theme.color.text_muted,
		},
	}
end
