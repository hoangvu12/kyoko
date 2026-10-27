return {
	on_action = function(key)
		if key == "read_all" then
			kyoko.actions.mark_all_read()
		end
	end,
}
