local REMOVED_LIMIT = 500
local REMOVE_KEY = "remove"

local order = kyoko.storage.get("removed")
if type(order) ~= "table" then
	order = {}
end
local removed = {}
for _, id in order do
	removed[id] = true
end

local function forget(id: string)
	if removed[id] then
		return
	end
	removed[id] = true
	table.insert(order, id)
	while #order > REMOVED_LIMIT do
		removed[table.remove(order, 1)] = nil
	end
	kyoko.storage.set("removed", order)
end

local function ignored(message, settings): boolean
	return (settings.ignore_bots and message.bot)
		or (settings.ignore_self and message.mine)
		or removed[message.id] == true
end

local function with_remove(look, settings)
	if settings.remove_button then
		look.accessory = { text = "Remove from log", action = REMOVE_KEY }
	end
	return look
end

local function history(message, settings)
	if settings.show == "deleted" or #message.versions == 0 then
		return nil
	end
	if settings.inline_edits then
		return "inline"
	end
	return "collapsed"
end

return {
	show_deleted = function()
		return kyoko.settings.get("show") ~= "edits"
	end,

	render_message = function(message)
		local settings = kyoko.settings.all()
		if message.deleted then
			if settings.show == "edits" or ignored(message, settings) then
				return { deleted = "hide" }
			end
			local style = if settings.deleted_style == "tint" then "tint" else "text"
			return with_remove({ deleted = style, history = history(message, settings) }, settings)
		end
		if ignored(message, settings) then
			return nil
		end
		local shown = history(message, settings)
		if shown == nil then
			return nil
		end
		return with_remove({ history = shown }, settings)
	end,

	on_action = function(key, context)
		if key == REMOVE_KEY and context.message_id then
			forget(context.message_id)
		end
	end,
}
