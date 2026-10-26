type Person = { username: string, name: string }

local function same(first: string, second: string): boolean
	return string.lower(first) == string.lower(second)
end

return {
	format_name = function(person: Person)
		local mode = kyoko.settings.get("mode")
		if mode == "user" then
			return { name = person.username }
		end
		if same(person.username, person.name) and kyoko.settings.get("hide_same") ~= false then
			return nil
		end
		if mode == "user-nick" then
			return { name = person.username, extra = person.name }
		end
		return { name = person.name, extra = person.username }
	end,
}
