return {
	on_action = function(key, context)
		if key ~= "toggle" or context.source ~= "slot" then
			return
		end
		local ok, refusal = pcall(kyoko.actions.share_activity)
		if not ok then
			kyoko.log("toggle refused:", refusal)
		end
	end,
}
