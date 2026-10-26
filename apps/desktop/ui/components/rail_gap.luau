local theme = require("../theme")

export type Props = {
	drop: string,
	target: string,
	on_drop: (payload: any, drop: string, target: string) -> (),
}

return ui.component(function(props: Props)
	local over = ui.use_state(false)
	return ui.row {
		absolute = true,
		top = 0,
		left = 0,
		right = 0,
		h = theme.size.space_xs,
		y = -theme.number(theme.size.space_xs)(),
		justify = "center",
		on_drag_over = function()
			over:set(true)
		end,
		on_drag_leave = function()
			over:set(false)
		end,
		on_drop = function(payload: any)
			over:set(false)
			props.on_drop(payload, props.drop, props.target)
		end,
		ui.div {
			w = theme.size.guildbar_avatar_size,
			h_full = true,
			rounded = theme.size.radius_xs,
			bg = function()
				return if over:get() then theme.color.accent_2 else "#00000000"
			end,
		},
	}
end)
