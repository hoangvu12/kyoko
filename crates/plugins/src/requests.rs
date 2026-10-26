use std::sync::Arc;

use state::Expression;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginRequest {
	Clone { plugin: String, expression: Expression },
}

pub(crate) type RequestSink = Arc<dyn Fn(PluginRequest) + Send + Sync>;
