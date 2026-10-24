wasmtime::component::bindgen!({
	path: "wit/kyoko.wit",
	world: "plugin",
	additional_derives: [PartialEq],
});
