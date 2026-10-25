local theme = require("../theme")

export type Props = {
	status: string,
	size: number,
	cutout: any,
}

local COLORS = {
	online = theme.color.status_online,
	idle = theme.color.status_idle,
	dnd = theme.color.status_dnd,
}

local function mark(props: Props)
	local size = props.size
	if props.status == "online" then
		return nil
	elseif props.status == "idle" then
		return ui.div {
			absolute = true,
			left = -size * 0.2,
			top = -size * 0.2,
			w = size * 0.7,
			h = size * 0.7,
			rounded = size,
			bg = props.cutout,
		}
	elseif props.status == "dnd" then
		return ui.div {
			w = size * 0.64,
			h = size * 0.26,
			rounded = size * 0.13,
			bg = props.cutout,
		}
	end
	return ui.div {
		w = size * 0.5,
		h = size * 0.5,
		rounded = size,
		bg = props.cutout,
	}
end

return function(props: Props)
	local size = props.size
	return ui.row {
		relative = true,
		w = size,
		h = size,
		shrink = 0,
		rounded = size,
		justify = "center",
		items = "center",
		bg = COLORS[props.status] or theme.color.status_offline,
		mark(props),
	}
end
