//! Pinned download targets for the local AI provider — ARCHITECTURE.md §10: a pinned
//! `llama-server` release per OS/arch, and a pinned GGUF model, both with a recorded sha256 taken
//! at pin time (verified against the actual file on disk, never trusted from the network alone).
//!
//! Bumping either pin means re-downloading the asset, re-hashing it, and updating the constants
//! here — there is deliberately no "latest" resolution at runtime (ARCHITECTURE.md §10 says
//! "pinned", not "auto-updating").

/// `ggml-org/llama.cpp` release tag these asset names and hashes were pinned against.
pub const LLAMA_CPP_RELEASE: &str = "b9874";

pub struct LlamaServerAsset {
    /// Asset file name under `https://github.com/ggml-org/llama.cpp/releases/download/{tag}/`.
    pub asset_name: &'static str,
    pub sha256: &'static str,
    /// Path to the `llama-server` executable once `asset_name` is extracted into its own
    /// directory — macOS/Linux archives nest everything under `llama-{tag}/`; the Windows zip is
    /// flat.
    pub binary_relative_path: &'static str,
}

fn download_url(asset_name: &str) -> String {
    format!(
        "https://github.com/ggml-org/llama.cpp/releases/download/{LLAMA_CPP_RELEASE}/{asset_name}"
    )
}

/// Picks the asset for the running OS/arch. `None` on a combination llama.cpp doesn't ship a
/// prebuilt CPU binary for (the local provider is then simply unavailable there — Ollama/Remote
/// still work).
pub fn current_platform_asset() -> Option<LlamaServerAsset> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some(LlamaServerAsset {
            asset_name: "llama-b9874-bin-macos-arm64.tar.gz",
            sha256: "6ad88c0f70c4731200e514132043b08894238beebf1a8e80e2b14a0ebecd1cb8",
            binary_relative_path: "llama-b9874/llama-server",
        }),
        ("macos", "x86_64") => Some(LlamaServerAsset {
            asset_name: "llama-b9874-bin-macos-x64.tar.gz",
            sha256: "ba4509c4b71bc6ff1abb00185c203967a8487c991500ccf4839c5ea5422cd1a6",
            binary_relative_path: "llama-b9874/llama-server",
        }),
        ("linux", "x86_64") => Some(LlamaServerAsset {
            asset_name: "llama-b9874-bin-ubuntu-x64.tar.gz",
            sha256: "5a3304b45428c12e8a81709b741d3770fa10d333d663c3c8039456fa9dd447bd",
            binary_relative_path: "llama-b9874/llama-server",
        }),
        ("linux", "aarch64") => Some(LlamaServerAsset {
            asset_name: "llama-b9874-bin-ubuntu-arm64.tar.gz",
            sha256: "33ad52ddaac26ffc965d41a4a485346ad57aa1a08c22916a47637dc273f007ec",
            binary_relative_path: "llama-b9874/llama-server",
        }),
        ("windows", "x86_64") => Some(LlamaServerAsset {
            asset_name: "llama-b9874-bin-win-cpu-x64.zip",
            sha256: "afeb33e219b54f5babddf31f31181ffa220a1c60600719d33633e78834393133",
            binary_relative_path: "llama-server.exe",
        }),
        ("windows", "aarch64") => Some(LlamaServerAsset {
            asset_name: "llama-b9874-bin-win-cpu-arm64.zip",
            sha256: "ec2274d05750e50797159e95ccde1e1c38c0dbad484d3aed8f59ef6098f7b54c",
            binary_relative_path: "llama-server.exe",
        }),
        _ => None,
    }
}

pub fn current_platform_asset_url() -> Option<String> {
    current_platform_asset().map(|a| download_url(a.asset_name))
}

/// SPEC-DEVIATION (DESIGN_SPEC.md §13): Jamie explicitly replaced the originally specified Gemma
/// 3 1B pin with Google's official QAT q4_0 GGUF of Gemma 4 E2B instruction-tuned (~3.3 GB).
/// Keeping the larger pin in this polish pass avoids silently changing a multi-gigabyte runtime
/// asset; the UI, changelog, URL, checksum, and size all describe the shipped pin consistently.
/// sha256 recorded at pin time from the repo's own LFS metadata
/// (`google/gemma-4-E2B-it-qat-q4_0-gguf`, file `gemma-4-E2B_q4_0-it.gguf`).
pub const MODEL_FILE_NAME: &str = "gemma-4-E2B_q4_0-it.gguf";
pub const MODEL_URL: &str =
    "https://huggingface.co/google/gemma-4-E2B-it-qat-q4_0-gguf/resolve/main/gemma-4-E2B_q4_0-it.gguf";
pub const MODEL_SHA256: &str = "3646b4c147cd235a44d91df1546d3b7d8e29b547dbe4e1f80856419aa455e6fd";
pub const MODEL_SIZE_BYTES: u64 = 3_349_514_112;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_platform_asset_resolves_on_supported_dev_platforms() {
        // This suite only runs on macOS/Linux/Windows x86_64/aarch64 CI runners, all of which
        // have a pinned asset above — a `None` here would mean this file drifted from reality.
        assert!(current_platform_asset().is_some());
    }

    #[test]
    fn model_sha256_is_64_hex_chars() {
        assert_eq!(MODEL_SHA256.len(), 64);
        assert!(MODEL_SHA256.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn download_url_is_well_formed() {
        let url = download_url("llama-b9874-bin-macos-arm64.tar.gz");
        assert_eq!(
            url,
            "https://github.com/ggml-org/llama.cpp/releases/download/b9874/llama-b9874-bin-macos-arm64.tar.gz"
        );
    }
}
