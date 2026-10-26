local CLONES = {
	clone_emoji = "custom_emoji",
	clone_sticker = "sticker",
}

return {
	on_action = function(key, context)
		if context.source ~= "menu" or CLONES[key] ~= context.target then
			return
		end
		local ok, refusal = pcall(kyoko.actions.clone_expression)
		if not ok then
			kyoko.log("clone refused:", refusal)
		end
	end,
}
