return {
	allow_typing = function()
		return kyoko.settings.get("silent") ~= true
	end,
}
