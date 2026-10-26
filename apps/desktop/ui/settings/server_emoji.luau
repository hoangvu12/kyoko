local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local kit = require("./kit")

local REQUIREMENTS = {
	"File type: JPEG, PNG, GIF, WEBP",
	"Kyoko scales the image to 128x128 for you",
	"Naming: Emoji names must be at least 2 characters long and can only contain alphanumeric characters and underscores",
}

local upload_name = ui.state("emoji_name", "")
local picking = false

local function n(token: string): number
	return theme.number(token)()
end

local function emoji()
	return state.server.emoji:get()
end

local function pick()
	if picking then
		return
	end
	picking = true
	ui.task(function()
		local ok, files = pcall(function()
			return ui.open_files({ title = "Upload Emoji", multiple = false }):await()
		end)
		picking = false
		if ok and files[1] then
			actions.server_action("upload_pick", files[1].path)
		end
	end)
end

local COLUMNS: { kit.Column } = {
	{ title = "Image", w = function()
		return n(theme.size.space_56)
	end },
	{ title = "Name", grow = 1 },
	{ title = "Uploaded by", grow = 1 },
}

local function image(src: string)
	local side = n(theme.size.avatar_size_sm)
	return ui.row {
		w = side,
		h = side,
		shrink = 0,
		rounded = theme.size.radius_sm,
		overflow = "hidden",
		if src ~= "" then ui.image { src = src, w = side, h = side, fit = "contain" } else nil,
	}
end

local function row(entry: state.ServerEmojiRow)
	return kit.table_row {
		id = "emoji_row_" .. entry.id,
		columns = COLUMNS,
		cells = {
			image(entry.image),
			kit.text(":" .. entry.name .. ":", theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_3, {
				truncate = true,
				min_w = 0,
			}),
			if entry.uploader ~= "" then kit.person(entry.avatar, entry.uploader, entry.color) else nil,
		},
		actions = function(hovered: () -> boolean)
			return kit.row_actions(
				hovered,
				kit.round_button {
					id = "emoji_delete_" .. entry.id,
					name = "close",
					tooltip = "Delete",
					tint = theme.color.red_2,
					on_click = function()
						actions.server_action("emoji_delete", { id = entry.id, name = entry.name })
					end,
				}
			)
		end,
	}
end

local function listed(animated: boolean): { state.ServerEmojiRow }
	local found = {}
	local value = emoji()
	for _, entry in (value and value.rows) or {} do
		if (entry.animated == true) == animated then
			table.insert(found, entry)
		end
	end
	return found
end

local function section(title: string, animated: boolean)
	local function left(): number
		local value = emoji()
		if not value then
			return 0
		end
		local used = if animated then value.animated else value.still
		return math.max(0, value.slots - used)
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		mt = theme.size.space_16,
		ui.rich {
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_label,
			color = theme.color.text_2,
			ui.span { title, literal = true, weight = theme.weight.semibold },
			ui.span {
				function()
					local count = left()
					return string.format(" — %d %s available", count, if count == 1 then "slot" else "slots")
				end,
				text_size = theme.size.font_size_sm,
				color = theme.color.text_4,
			},
		},
		ui.column {
			w_full = true,
			kit.table_head(COLUMNS),
			ui.each(ui.computed(function()
				return listed(animated)
			end), row, {
				key = function(entry: state.ServerEmojiRow)
					return entry.id .. "|" .. entry.name .. "|" .. entry.image .. "|" .. entry.avatar
				end,
				w_full = true,
			}),
		},
	}
end

local function form()
	return ui.presence(function()
		local value = emoji()
		return if value and value.upload then value.upload.serial else nil
	end, function()
		local value = emoji()
		local upload = if value then value.upload else nil
		upload_name:set(if upload then upload.name else "")
		local side = n(theme.size.space_56)
		return ui.row {
			id = "emoji_upload_form",
			w_full = true,
			gap = theme.size.space_12,
			p = theme.size.space_12,
			items = "center",
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border,
			ui.row {
				w = side,
				h = side,
				shrink = 0,
				if upload and upload.preview ~= "" then ui.image { src = upload.preview, w = side, h = side, fit = "contain" } else nil,
			},
			ui.div {
				grow = 1,
				min_w = 0,
				kit.field("Emoji Name", kit.input(upload_name, "", { id = "emoji_upload_name", label = "Emoji Name" })),
			},
			kit.button {
				text = "Cancel",
				on_click = function()
					actions.server_action("upload_cancel")
				end,
			},
			kit.button {
				id = "emoji_upload_send",
				text = "Upload",
				tone = "primary",
				on_click = function()
					if #upload_name:peek() >= 2 then
						actions.server_action("upload_send", { name = upload_name:peek() })
					end
				end,
			},
		}
	end)
end

local function build()
	local bullets = {}
	for _, line in REQUIREMENTS do
		table.insert(
			bullets,
			ui.row {
				w_full = true,
				gap = theme.size.space_8,
				kit.hint("•"),
				ui.div { grow = 1, min_w = 0, kit.hint(line) },
			}
		)
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		kit.title("Emoji"),
		kit.hint(function()
			local value = emoji()
			return string.format(
				"Add up to %d custom emoji that anyone can use in this server. Animated GIF emoji may be used by members with Discord Nitro.",
				if value then value.slots else 50
			)
		end),
		kit.eyebrow("Upload requirements"),
		ui.column { w_full = true, pl = theme.size.space_8, table.unpack(bullets) },
		ui.row {
			kit.button {
				id = "emoji_upload",
				text = "Upload Emoji",
				tone = "primary",
				height = theme.size.button_height_md,
				on_click = pick,
			},
		},
		form(),
		section("Emoji", false),
		ui.show(function()
			return #listed(true) > 0
		end, section("Animated Emoji", true)),
		ui.show(function()
			local value = emoji()
			return value ~= nil and #value.rows == 0
		end, kit.empty("emoji", "This server has no custom emoji yet.")),
	}
end

return {
	keywords = "emoji emojis custom upload expressions",
	build = build,
}
