return {
	decorate_user = function(user: { owner: boolean })
		if user.owner then
			return { crown = true }
		end
		return nil
	end,
}
