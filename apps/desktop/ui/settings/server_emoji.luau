local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local widgets = require("@settings/widgets")
local kit = require("@settings/server_kit")

local IMAGE = 56
local UPLOADER = 16

local upload_name = ui.state("server.emoji_name", "")
local picking = false

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

local card = ui.component(function(props: { entry: state.ServerEmojiRow })
	local entry = props.entry
	local hovered = ui.use_state(false)
	return ui.row {
		w = theme.number(theme.size.manage_grid_cell)() * 3,
		gap = theme.size.space_12,
		p = theme.size.space_12,
		items = "center",
		rounded = theme.size.radius_md,
		bg = theme.color.background_secondary,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		kit.image(entry.image, IMAGE, IMAGE),
		ui.column {
			grow = 1,
			min_w = 0,
			gap = theme.size.space_4,
			widgets.label(":" .. entry.name .. ":", theme.size.font_md, theme.color.header_primary, {
				weight = theme.weight.semibold,
				literal = true,
				truncate = true,
			}),
			if entry.uploader ~= ""
				then ui.row {
					items = "center",
					gap = theme.size.space_4,
					avatar { src = entry.avatar, size = UPLOADER },
					widgets.label(entry.uploader, theme.size.font_tiny, theme.color.text_muted, {
						literal = true,
						truncate = true,
					}),
				}
				else nil,
		},
		ui.show(function()
			return hovered:get()
		end, kit.icon_button {
			name = "trash",
			tooltip = "Delete",
			on_click = function()
				actions.server_action("emoji_delete", { id = entry.id, name = entry.name })
			end,
		}),
	}
end)

local function form()
	return ui.presence(function()
		local value = emoji()
		return if value and value.upload then value.upload.serial else nil
	end, function()
		local value = emoji()
		local upload = if value then value.upload else nil
		upload_name:set(if upload then upload.name else "")
		return ui.row {
			w_full = true,
			shrink = 0,
			gap = theme.size.space_12,
			mb = theme.size.space_16,
			p = theme.size.space_12,
			items = "center",
			rounded = theme.size.radius_md,
			bg = theme.color.background_secondary,
			kit.image(if upload then upload.preview else "", IMAGE, IMAGE),
			ui.div { grow = 1, min_w = 0, kit.input(upload_name, "") },
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
						actions.server_action("upload_send", { name = upload_name:peek() })
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
		kit.title("Emoji"),
		ui.row {
			w_full = true,
			shrink = 0,
			items = "center",
			justify = "between",
			pb = theme.size.space_16,
			ui.column {
				kit.caption(function()
					local value = emoji()
					return string.format("%d of %d static emoji", if value then value.still else 0, if value then value.slots else 50)
				end),
				kit.caption(function()
					local value = emoji()
					return string.format(
						"%d of %d animated emoji",
						if value then value.animated else 0,
						if value then value.slots else 50
					)
				end),
			},
			kit.button { text = "Upload Emoji", icon = "upload", on_click = pick },
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
					local value = emoji()
					return if value then value.rows else {}
				end), function(entry: state.ServerEmojiRow)
					return card { entry = entry }
				end, {
					key = function(entry: state.ServerEmojiRow)
						return entry.id
					end,
					w_full = true,
					direction = "row",
					flex_wrap = true,
					gap = theme.size.space_12,
				}),
				ui.show(function()
					local value = emoji()
					return value ~= nil and #value.rows == 0
				end, kit.empty("emoji", "This server has no custom emoji yet.")),
			},
		},
	}
end
