local theme = require("../theme")

export type Props = {
	size: number,
	rounded: any?,
}

local SMALL_UP_TO = 20

return function(props: Props)
	return ui.image {
		src = if props.size <= SMALL_UP_TO then "images/kyoko_small.png" else "images/kyoko.png",
		w = props.size,
		h = props.size,
		shrink = 0,
		fit = "cover",
		rounded = props.rounded or theme.size.radius_xs,
	}
end
