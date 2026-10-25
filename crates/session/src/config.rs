use std::sync::Arc;

use gateway::GatewayTuning;
use protocol::Token;
use rest::ClientIdentity;
use rest::RestTuning;
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
	pub identity: ClientIdentity,
	pub build_source: Option<Url>,
	pub runtime: Option<Handle>,
	pub notify: NotifyFn,
	pub hooks: Option<Arc<dyn SessionHooks>>,
	pub gateway: GatewayTuning,
	pub rest: RestTuning,
	pub remember_account: bool,
}
