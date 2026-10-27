local theme = require("../theme")
local status_dot = require("./status_dot")

export type Props = {
	src: string,
	size: number,
	status: string?,
	cutout: any?,
	moving: string?,
	playing: (() -> boolean)?,
}

local function badge(props: Props, status: string)
	local outer = math.clamp(props.size * 0.5, 10, 24)
	local ring = math.floor(outer * 0.19 + 0.5)
	local cutout = props.cutout or theme.color.background_secondary
	return ui.row {
		absolute = true,
		right = -outer * 0.25,
		bottom = -outer * 0.25,
		w = outer,
		h = outer,
		rounded = outer,
		justify = "center",
		items = "center",
		bg = cutout,
		status_dot { status = status, size = outer - ring * 2, cutout = cutout },
	}
end

return function(props: Props)
	local size = props.size
	local status = props.status
	return ui.div {
		relative = true,
		w = size,
		h = size,
		shrink = 0,
		ui.div {
			w = size,
			h = size,
			rounded = size,
			overflow = "hidden",
			bg = theme.color.background_tertiary,
			ui.show(
				props.src ~= "",
				ui.image { src = props.src, w = size, h = size, fit = "cover" }
			),
			if props.moving and props.moving ~= "" and props.playing
				then ui.show(
					props.playing,
					ui.image { src = props.moving, absolute = true, top = 0, left = 0, w = size, h = size, fit = "cover" }
				)
				else nil,
		},
		if status then badge(props, status) else nil,
	}
end
