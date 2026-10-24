use std::sync::Arc;

use protocol::Token;
use protocol::gateway::IdentifyProperties;
use state::Changes;
use tokio::runtime::Handle;
use url::Url;

use crate::hooks::SessionHooks;

pub type NotifyFn = Arc<dyn Fn(Changes) + Send + Sync>;

pub struct SessionConfig {
	pub store: Arc<store::Store>,
	pub rest_base: Url,
	pub gateway_url: String,
	pub token: Token,
	pub identify: IdentifyProperties,
	pub locale: String,
	pub runtime: Option<Handle>,
	pub notify: NotifyFn,
	pub hooks: Option<Arc<dyn SessionHooks>>,
}
