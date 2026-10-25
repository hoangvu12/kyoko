local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local widgets = require("@settings/widgets")
local kit = require("@settings/server_kit")

local PREVIEW = 56

local upload_name = ui.state("server.sticker_name", "")
local upload_description = ui.state("server.sticker_description", "")
local upload_tags = ui.state("server.sticker_tags", "")
local picking = false

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

local function card(entry: state.ServerStickerRow)
	local side = theme.number(theme.size.picker_sticker)()
	return ui.column {
		w = theme.number(theme.size.manage_grid_cell)() + theme.number(theme.size.space_32)(),
		items = "center",
		gap = theme.size.space_8,
		p = theme.size.space_12,
		rounded = theme.size.radius_md,
		bg = theme.color.background_secondary,
		ui.row {
			w = side,
			h = side,
			justify = "center",
			items = "center",
			if entry.image ~= ""
				then kit.image(entry.image, side, side)
				else icon { name = "sticker", size = 48, color = theme.color.interactive_muted },
		},
		widgets.label(entry.name, theme.size.font_md, theme.color.header_primary, {
			weight = theme.weight.semibold,
			literal = true,
			truncate = true,
			align = "center",
			w_full = true,
		}),
		if entry.tags ~= ""
			then widgets.label(entry.tags, theme.size.font_tiny, theme.color.text_muted, {
				literal = true,
				truncate = true,
				align = "center",
				w_full = true,
			})
			else nil,
		ui.row {
			items = "center",
			gap = theme.size.space_4,
			kit.icon_button {
				name = "edit",
				tooltip = "Edit",
				on_click = function()
					actions.server_action("sticker_edit", entry.id)
				end,
			},
			kit.icon_button {
				name = "trash",
				tooltip = "Delete",
				tint = theme.color.danger,
				on_click = function()
					actions.server_action("sticker_delete", { id = entry.id, name = entry.name })
				end,
			},
		},
	}
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
		return ui.row {
			w_full = true,
			shrink = 0,
			gap = theme.size.space_12,
			mb = theme.size.space_16,
			p = theme.size.space_12,
			items = "center",
			rounded = theme.size.radius_md,
			bg = theme.color.background_secondary,
			kit.image(if upload then upload.preview else "", PREVIEW, PREVIEW),
			ui.column {
				grow = 1,
				min_w = 0,
				gap = theme.size.space_8,
				kit.field("Name", kit.input(upload_name, "")),
				kit.field("Description", kit.input(upload_description, "What does this sticker show?")),
				kit.field("Related emoji or tags", kit.input(upload_tags, "wave, hello")),
			},
			kit.button {
				text = "Cancel",
				tone = "ghost",
				on_click = function()
					actions.server_action("upload_cancel")
				end,
			},
			kit.button {
				text = "Upload",
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
		}
	end)
end

return function()
	return ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		kit.title("Stickers"),
		ui.row {
			w_full = true,
			shrink = 0,
			items = "center",
			justify = "between",
			pb = theme.size.space_16,
			kit.caption(function()
				local value = stickers()
				return string.format(
					"%d of %d sticker slots used",
					if value then value.count else 0,
					if value then value.slots else 5
				)
			end),
			kit.button { text = "Upload Sticker", icon = "upload", on_click = pick },
		},
		form(),
		ui.scroll {
			w_full = true,
			grow = 1,
			min_h = 0,
			ui.column {
				w_full = true,
				pb = theme.size.space_32,
				ui.each(ui.computed(function()
					local value = stickers()
					return if value then value.rows else {}
				end), card, {
					key = function(entry: state.ServerStickerRow)
						return entry.id
					end,
					w_full = true,
					direction = "row",
					flex_wrap = true,
					gap = theme.size.space_12,
				}),
				ui.show(function()
					local value = stickers()
					return value ~= nil and #value.rows == 0
				end, kit.empty("sticker", "This server has no stickers yet.")),
			},
		},
	}
end
