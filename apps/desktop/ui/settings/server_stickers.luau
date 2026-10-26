local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local kit = require("./kit")

local COLUMNS = 4

local upload_name = ui.state("sticker_name", "")
local upload_description = ui.state("sticker_description", "")
local upload_tags = ui.state("sticker_tags", "")
local picking = false

local function n(token: string): number
	return theme.number(token)()
end

local function stickers()
	return state.server.stickers:get()
end

local function pick()
	if picking then
		return
	end
	picking = true
	ui.task(function()
		local ok, files = pcall(function()
			return ui.open_files({ title = "Upload Sticker", multiple = false }):await()
		end)
		picking = false
		if ok and files[1] then
			actions.server_action("upload_pick", files[1].path)
		end
	end)
end

local function picture_side(): number
	return n(theme.size.space_56) + n(theme.size.space_24)
end

local card = ui.component(function(props: { entry: state.ServerStickerRow })
	local entry = props.entry
	local hovered = ui.use_state(false)
	local side = picture_side()
	return ui.column {
		id = "sticker_card_" .. entry.id,
		relative = true,
		grow = 1,
		basis = 0,
		min_w = 0,
		items = "center",
		gap = theme.size.space_8,
		p = theme.size.space_12,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		ui.row {
			w = side,
			h = side,
			justify = "center",
			items = "center",
			rounded = theme.size.radius_md,
			overflow = "hidden",
			if entry.image ~= ""
				then ui.image { src = entry.image, w = side, h = side, fit = "contain" }
				else icon { name = "sticker", size = theme.size.icon_size_lg :: any, color = theme.color.text_5 },
		},
		kit.text(entry.name, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_2, {
			weight = theme.weight.semibold,
			truncate = true,
			min_w = 0,
		}),
		ui.show(function()
			return hovered:get()
		end, ui.row {
			absolute = true,
			top = n(theme.size.space_8),
			right = n(theme.size.space_8),
			gap = theme.size.space_4,
			kit.round_button {
				id = "sticker_edit_" .. entry.id,
				name = "edit",
				tooltip = "Edit",
				on_click = function()
					actions.server_action("sticker_edit", entry.id)
				end,
			},
			kit.round_button {
				id = "sticker_delete_" .. entry.id,
				name = "close",
				tooltip = "Delete",
				tint = theme.color.red_2,
				on_click = function()
					actions.server_action("sticker_delete", { id = entry.id, name = entry.name })
				end,
			},
		}),
	}
end)

local function upload_slot()
	return ui.button {
		id = "sticker_upload_slot",
		label = "Upload Sticker",
		grow = 1,
		basis = 0,
		min_w = 0,
		min_h = n(theme.size.space_56) * 2 + n(theme.size.space_24),
		items = "center",
		justify = "center",
		rounded = theme.size.radius_sm,
		border = theme.size.space_2,
		border_color = theme.color.border,
		focus = { border = theme.size.space_2, border_color = theme.color.accent_2 },
		bg = "#00000000",
		hover = { bg = theme.color.hover },
		cursor = "pointer",
		on_click = pick,
		ui.column {
			items = "center",
			gap = theme.size.space_8,
			icon { name = "image_plus", size = theme.size.icon_size_md :: any, color = theme.color.text_4 },
			kit.text("Upload", theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_4, {
				weight = theme.weight.semibold,
			}),
		},
	}
end

local function grid_key(): string
	local value = stickers()
	if not value then
		return ""
	end
	local parts = { tostring(value.count < value.slots) }
	for _, entry in value.rows do
		table.insert(parts, entry.id .. ":" .. entry.name .. ":" .. entry.image)
	end
	return table.concat(parts, "|")
end

local function grid()
	return ui.div { w_full = true, ui.presence(grid_key, function()
		local value = stickers()
		local cells = {}
		for _, entry in (value and value.rows) or {} do
			table.insert(cells, card { entry = entry })
		end
		if value and value.count < value.slots then
			table.insert(cells, upload_slot())
		end
		local lines = {}
		for first = 1, #cells, COLUMNS do
			local line = {}
			for index = first, first + COLUMNS - 1 do
				table.insert(line, cells[index] or ui.div { grow = 1, basis = 0, min_w = 0 })
			end
			table.insert(lines, ui.row { w_full = true, gap = theme.size.space_12, items = "stretch", table.unpack(line) })
		end
		return ui.column { w_full = true, gap = theme.size.space_12, table.unpack(lines) }
	end) }
end

local function form()
	return ui.presence(function()
		local value = stickers()
		return if value and value.upload then value.upload.serial else nil
	end, function()
		local value = stickers()
		local upload = if value then value.upload else nil
		upload_name:set(if upload then upload.name else "")
		upload_description:set("")
		upload_tags:set("")
		local side = picture_side()
		return ui.row {
			id = "sticker_upload_form",
			w_full = true,
			gap = theme.size.space_16,
			p = theme.size.space_16,
			items = "start",
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
			ui.column {
				grow = 1,
				min_w = 0,
				gap = theme.size.space_12,
				kit.field("Sticker Name", kit.input(upload_name, "", { id = "sticker_upload_name", label = "Sticker Name" })),
				kit.field("Description", kit.input(upload_description, "What does this sticker show?")),
				kit.field("Related Emoji or Tags", kit.input(upload_tags, "wave, hello")),
				ui.row {
					w_full = true,
					justify = "end",
					gap = theme.size.space_8,
					kit.button {
						text = "Cancel",
						on_click = function()
							actions.server_action("upload_cancel")
						end,
					},
					kit.button {
						id = "sticker_upload_send",
						text = "Upload",
						tone = "primary",
						on_click = function()
							if #upload_name:peek() >= 2 then
								actions.server_action("upload_send", {
									name = upload_name:peek(),
									description = upload_description:peek(),
									tags = upload_tags:peek(),
								})
							end
						end,
					},
				},
			},
		}
	end)
end

local function slots_title(): string
	local value = stickers()
	local slots = if value then value.slots else 5
	local tier = if value then value.tier or 0 else 0
	local count = string.format("%d sticker %s", slots, if slots == 1 then "slot" else "slots")
	return if tier > 0 then string.format("Level %d · %s", tier, count) else count
end

local function build()
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		kit.title("Stickers"),
		kit.hint("Stickers can be used by anyone in this server. Boost the server to unlock more sticker slots."),
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_16,
			p = theme.size.space_16,
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border,
			icon { name = "sparkles", size = theme.size.icon_size_md :: any, color = theme.color.accent_2 },
			kit.label(slots_title, function()
				local value = stickers()
				return string.format(
					"%d of %d slots used",
					if value then value.count else 0,
					if value then value.slots else 5
				)
			end),
			kit.button {
				id = "sticker_upload",
				text = "Upload Sticker",
				tone = "primary",
				height = theme.size.button_height_md,
				on_click = pick,
			},
		},
		form(),
		grid(),
	}
end

return {
	keywords = "stickers sticker upload expressions boost",
	build = build,
}
