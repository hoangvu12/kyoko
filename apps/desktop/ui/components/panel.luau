local theme = require("../theme")

return function(props: { [string]: any })
	props.bg = props.bg or theme.color.bg_4
	props.border = theme.size.border_thickness
	props.border_color = theme.color.border
	props.rounded = theme.size.radius_lg
	props.overflow = "hidden"
	props.shrink = props.shrink or 0
	return ui.column(props)
end
