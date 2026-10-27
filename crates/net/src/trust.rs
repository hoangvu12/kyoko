use wreq::ClientBuilder;
use wreq::tls::trust::CertStore;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Trust {
	#[default]
	Bundled,
	Only(Vec<String>),
}

impl Trust {
	pub fn only(root_pem: impl Into<String>) -> Trust {
		Trust::Only(vec![root_pem.into()])
	}

	pub(crate) fn apply(&self, builder: ClientBuilder) -> wreq::Result<ClientBuilder> {
		match self {
			Trust::Bundled => Ok(builder),
			Trust::Only(roots) => {
				let store = CertStore::from_pem_certs(roots.iter().map(String::as_bytes))?;
				Ok(builder.tls_cert_store(store))
			}
		}
	}
}
