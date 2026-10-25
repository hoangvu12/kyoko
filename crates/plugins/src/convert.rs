use mlua::{Lua, Table, Value};
use serde_json::{Map, Value as Json};

const DEPTH_LIMIT: usize = 16;

pub(crate) fn to_json(value: &Value) -> Result<Json, String> {
	to_json_at(value, 0)
}

fn to_json_at(value: &Value, depth: usize) -> Result<Json, String> {
	if depth > DEPTH_LIMIT {
		return Err("the value is nested too deeply".to_owned());
	}
	Ok(match value {
		Value::Nil => Json::Null,
		Value::Boolean(flag) => Json::Bool(*flag),
		Value::Integer(integer) => Json::from(*integer),
		Value::Number(number) => serde_json::Number::from_f64(*number)
			.map(Json::Number)
			.ok_or_else(|| "numbers must be finite".to_owned())?,
		Value::String(text) => Json::String(
			text.to_str()
				.map_err(|_| "text must be UTF-8".to_owned())?
				.to_owned(),
		),
		Value::Table(table) => table_to_json(table, depth)?,
		_ => return Err("only nil, booleans, numbers, text and tables can be kept".to_owned()),
	})
}

fn table_to_json(table: &Table, depth: usize) -> Result<Json, String> {
	let length = table.raw_len();
	let mut pairs = Vec::new();
	for pair in table.pairs::<Value, Value>() {
		pairs.push(pair.map_err(|error| error.to_string())?);
	}
	let is_list = pairs.len() == length
		&& pairs.iter().all(
			|(key, _)| matches!(key, Value::Integer(index) if *index >= 1 && (*index as usize) <= length),
		);
	if is_list && length > 0 {
		let mut items = vec![Json::Null; length];
		for (key, value) in &pairs {
			if let Value::Integer(index) = key {
				items[*index as usize - 1] = to_json_at(value, depth + 1)?;
			}
		}
		return Ok(Json::Array(items));
	}
	let mut map = Map::new();
	for (key, value) in &pairs {
		let key = match key {
			Value::String(text) => text
				.to_str()
				.map_err(|_| "keys must be UTF-8".to_owned())?
				.to_owned(),
			Value::Integer(integer) => integer.to_string(),
			_ => return Err("table keys must be text or whole numbers".to_owned()),
		};
		map.insert(key, to_json_at(value, depth + 1)?);
	}
	Ok(Json::Object(map))
}

pub(crate) fn to_lua(lua: &Lua, value: &Json) -> mlua::Result<Value> {
	Ok(match value {
		Json::Null => Value::Nil,
		Json::Bool(flag) => Value::Boolean(*flag),
		Json::Number(number) => match number.as_i64() {
			Some(integer) if integer.unsigned_abs() < (1 << 53) => Value::Integer(integer),
			_ => Value::Number(number.as_f64().unwrap_or_default()),
		},
		Json::String(text) => Value::String(lua.create_string(text)?),
		Json::Array(items) => {
			let table = lua.create_table_with_capacity(items.len(), 0)?;
			for item in items {
				table.raw_push(to_lua(lua, item)?)?;
			}
			Value::Table(table)
		}
		Json::Object(map) => {
			let table = lua.create_table_with_capacity(0, map.len())?;
			for (key, item) in map {
				table.raw_set(key.as_str(), to_lua(lua, item)?)?;
			}
			Value::Table(table)
		}
	})
}
