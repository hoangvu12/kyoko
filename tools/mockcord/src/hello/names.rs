pub const SERVER_NAME: u16 = 0x0000;
pub const STATUS_REQUEST: u16 = 0x0005;
pub const SUPPORTED_GROUPS: u16 = 0x000a;
pub const EC_POINT_FORMATS: u16 = 0x000b;
pub const SIGNATURE_ALGORITHMS: u16 = 0x000d;
pub const USE_SRTP: u16 = 0x000e;
pub const ALPN: u16 = 0x0010;
pub const PADDING: u16 = 0x0015;
pub const COMPRESS_CERTIFICATE: u16 = 0x001b;
pub const PRE_SHARED_KEY: u16 = 0x0029;
pub const SUPPORTED_VERSIONS: u16 = 0x002b;
pub const PSK_KEY_EXCHANGE_MODES: u16 = 0x002d;
pub const KEY_SHARE: u16 = 0x0033;
pub const ALPS_OLD: u16 = 0x4469;
pub const ALPS: u16 = 0x44cd;
pub const TRUST_ANCHORS: u16 = 0xca34;
pub const ENCRYPTED_CLIENT_HELLO: u16 = 0xfe0d;

pub fn is_grease(value: u16) -> bool {
	value & 0x0f0f == 0x0a0a && value >> 8 == value & 0xff
}

pub fn hex(value: u16) -> String {
	format!("0x{value:04x}")
}

fn named(value: u16, name: Option<&str>) -> String {
	if is_grease(value) {
		return "GREASE".to_owned();
	}
	name.map_or_else(|| hex(value), str::to_owned)
}

pub fn cipher(value: u16) -> String {
	let name = match value {
		0x1301 => Some("TLS_AES_128_GCM_SHA256"),
		0x1302 => Some("TLS_AES_256_GCM_SHA384"),
		0x1303 => Some("TLS_CHACHA20_POLY1305_SHA256"),
		0xc02b => Some("TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256"),
		0xc02f => Some("TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"),
		0xc02c => Some("TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384"),
		0xc030 => Some("TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384"),
		0xcca9 => Some("TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256"),
		0xcca8 => Some("TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256"),
		0xc009 => Some("TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA"),
		0xc00a => Some("TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA"),
		0xc013 => Some("TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA"),
		0xc014 => Some("TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA"),
		0x009c => Some("TLS_RSA_WITH_AES_128_GCM_SHA256"),
		0x009d => Some("TLS_RSA_WITH_AES_256_GCM_SHA384"),
		0x002f => Some("TLS_RSA_WITH_AES_128_CBC_SHA"),
		0x0035 => Some("TLS_RSA_WITH_AES_256_CBC_SHA"),
		_ => None,
	};
	named(value, name)
}

pub fn extension(value: u16) -> String {
	let name = match value {
		SERVER_NAME => Some("server_name"),
		STATUS_REQUEST => Some("status_request"),
		SUPPORTED_GROUPS => Some("supported_groups"),
		EC_POINT_FORMATS => Some("ec_point_formats"),
		SIGNATURE_ALGORITHMS => Some("signature_algorithms"),
		USE_SRTP => Some("use_srtp"),
		ALPN => Some("application_layer_protocol_negotiation"),
		0x0012 => Some("signed_certificate_timestamp"),
		PADDING => Some("padding"),
		0x0016 => Some("encrypt_then_mac"),
		0x0017 => Some("extended_master_secret"),
		COMPRESS_CERTIFICATE => Some("compress_certificate"),
		0x001c => Some("record_size_limit"),
		0x0023 => Some("session_ticket"),
		PRE_SHARED_KEY => Some("pre_shared_key"),
		0x002a => Some("early_data"),
		SUPPORTED_VERSIONS => Some("supported_versions"),
		PSK_KEY_EXCHANGE_MODES => Some("psk_key_exchange_modes"),
		KEY_SHARE => Some("key_share"),
		ALPS_OLD => Some("application_settings_old"),
		ALPS => Some("application_settings"),
		TRUST_ANCHORS => Some("trust_anchors"),
		ENCRYPTED_CLIENT_HELLO => Some("encrypted_client_hello"),
		0xff01 => Some("renegotiation_info"),
		_ => None,
	};
	named(value, name)
}

pub fn group(value: u16) -> String {
	let name = match value {
		0x0017 => Some("P-256"),
		0x0018 => Some("P-384"),
		0x0019 => Some("P-521"),
		0x001d => Some("X25519"),
		0x11ec => Some("X25519MLKEM768"),
		0x6399 => Some("X25519Kyber768Draft00"),
		_ => None,
	};
	named(value, name)
}

pub fn signature(value: u16) -> String {
	let name = match value {
		0x0403 => Some("ecdsa_secp256r1_sha256"),
		0x0503 => Some("ecdsa_secp384r1_sha384"),
		0x0603 => Some("ecdsa_secp521r1_sha512"),
		0x0804 => Some("rsa_pss_rsae_sha256"),
		0x0805 => Some("rsa_pss_rsae_sha384"),
		0x0806 => Some("rsa_pss_rsae_sha512"),
		0x0401 => Some("rsa_pkcs1_sha256"),
		0x0501 => Some("rsa_pkcs1_sha384"),
		0x0601 => Some("rsa_pkcs1_sha512"),
		0x0201 => Some("rsa_pkcs1_sha1"),
		0x0807 => Some("ed25519"),
		0x0904 => Some("mldsa44"),
		0x0905 => Some("mldsa65"),
		0x0906 => Some("mldsa87"),
		_ => None,
	};
	named(value, name)
}

pub fn version(value: u16) -> String {
	let name = match value {
		0x0304 => Some("TLS1.3"),
		0x0303 => Some("TLS1.2"),
		0x0302 => Some("TLS1.1"),
		0x0301 => Some("TLS1.0"),
		0xfeff => Some("DTLS1.0"),
		0xfefd => Some("DTLS1.2"),
		0xfefc => Some("DTLS1.3"),
		_ => None,
	};
	named(value, name)
}

pub fn certificate_compression(value: u16) -> String {
	let name = match value {
		1 => Some("zlib"),
		2 => Some("brotli"),
		3 => Some("zstd"),
		_ => None,
	};
	named(value, name)
}

pub fn setting(id: u16) -> String {
	match id {
		1 => "HEADER_TABLE_SIZE".to_owned(),
		2 => "ENABLE_PUSH".to_owned(),
		3 => "MAX_CONCURRENT_STREAMS".to_owned(),
		4 => "INITIAL_WINDOW_SIZE".to_owned(),
		5 => "MAX_FRAME_SIZE".to_owned(),
		6 => "MAX_HEADER_LIST_SIZE".to_owned(),
		8 => "ENABLE_CONNECT_PROTOCOL".to_owned(),
		9 => "NO_RFC7540_PRIORITIES".to_owned(),
		other => hex(other),
	}
}
