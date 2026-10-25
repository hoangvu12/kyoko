local theme = require("../theme")
local motion = require("./motion")

export type Props = {
	height: any,
	inset: number?,
}

return function(props: Props)
	return ui.row {
		absolute = true,
		left = props.inset or 0,
		top = 0,
		h_full = true,
		items = "center",
		ui.div {
			w = 4,
			h = ui.tween(props.height, { duration = motion.duration(160), ease = "outQuint" }),
			rounded_tr = 4,
			rounded_br = 4,
			bg = theme.color.unread_pill,
		},
	}
end
