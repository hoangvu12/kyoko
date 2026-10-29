use std::env;

const ROOT: &str = "csrc/rnnoise";
const SOURCES: [&str; 10] = [
	"denoise.c",
	"rnn.c",
	"pitch.c",
	"kiss_fft.c",
	"celt_lpc.c",
	"nnet.c",
	"nnet_default.c",
	"parse_lpcnet_weights.c",
	"rnnoise_tables.c",
	"rnnoise_data.c",
];
const X86_SOURCES: [&str; 2] = ["x86/x86cpu.c", "x86/x86_dnn_map.c"];

fn base(x86: bool) -> cc::Build {
	let mut build = cc::Build::new();
	build
		.include(format!("{ROOT}/include"))
		.include(format!("{ROOT}/src"))
		.define("USE_WEIGHTS_FILE", None)
		.define("DISABLE_DEBUG_FLOAT", None)
		.define("RNNOISE_BUILD", None)
		.warnings(false)
		.opt_level(3);
	if x86 {
		build
			.define("RNN_ENABLE_X86_RTCD", None)
			.define("OPUS_X86_MAY_HAVE_SSE", None)
			.define("OPUS_X86_MAY_HAVE_SSE2", None);
	}
	build
}

fn kernel(source: &str, library: &str, msvc_flags: &[&str], gnu_flags: &[&str], defines: &[&str]) {
	let mut build = base(true);
	let msvc = build.get_compiler().is_like_msvc();
	for flag in if msvc { msvc_flags } else { gnu_flags } {
		build.flag(*flag);
	}
	for define in defines {
		build.define(define, None);
	}
	build.file(format!("{ROOT}/src/{source}")).compile(library);
}

fn main() {
	println!("cargo:rerun-if-changed={ROOT}");
	let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
	let x86 = arch == "x86_64";
	let mut build = base(x86);
	for source in SOURCES {
		build.file(format!("{ROOT}/src/{source}"));
	}
	if x86 {
		for source in X86_SOURCES {
			build.file(format!("{ROOT}/src/{source}"));
		}
	}
	build.compile("rnnoise");
	if x86 {
		kernel(
			"x86/nnet_sse4_1.c",
			"rnnoise_sse4_1",
			&[],
			&["-msse4.1"],
			&["OPUS_X86_MAY_HAVE_SSE4_1"],
		);
		kernel(
			"x86/nnet_avx2.c",
			"rnnoise_avx2",
			&["/arch:AVX2"],
			&["-mavx", "-mfma", "-mavx2"],
			&[],
		);
	}
}
