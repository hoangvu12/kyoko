local theme = require("../theme")
local motion = require("./motion")

local PILL_MORPH = 160

export type Props = {
	height: any,
}

return function(props: Props)
	return ui.row {
		absolute = true,
		left = theme.number(theme.size.guild_pill_inset),
		top = 0,
		h_full = true,
		items = "center",
		ui.div {
			w = theme.size.guild_pill_width,
			h = ui.tween(props.height, { duration = motion.duration(PILL_MORPH), ease = "outQuint" }),
			rounded = theme.size.guild_pill_width,
			bg = theme.color.text_2,
		},
	}
end
