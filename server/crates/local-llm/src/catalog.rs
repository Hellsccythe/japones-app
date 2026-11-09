//! Models the app offers to download. Sizes and SHA-256 come from the Hugging
//! Face API and are checked after every download (a resumed download once
//! produced a longer, corrupt file that still loaded and generated garbage).

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    pub license: &'static str,
    /// Short hint shown next to the model ("PC", "celular", ...).
    pub recommended_for: &'static str,
}

pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "gemma-4-e4b",
        name: "Gemma 4 E4B",
        file: "gemma-4-E4B_q4_0-it.gguf",
        url: "https://huggingface.co/google/gemma-4-E4B-it-qat-q4_0-gguf/resolve/main/gemma-4-E4B_q4_0-it.gguf",
        size: 5_154_941_280,
        sha256: "676c35070db6dbe52f93e9c864ee0fba4eddea94b9c875d9cb10daff453fbaee",
        license: "Apache-2.0",
        recommended_for: "PC (melhor qualidade)",
    },
    ModelInfo {
        id: "gemma-4-e2b",
        name: "Gemma 4 E2B",
        file: "gemma-4-E2B_q4_0-it.gguf",
        url: "https://huggingface.co/google/gemma-4-E2B-it-qat-q4_0-gguf/resolve/main/gemma-4-E2B_q4_0-it.gguf",
        size: 3_349_516_256,
        sha256: "fa401b55b07ee70a54c6dae3903c783a6e65064312529ea57175cb5f8dec6634",
        license: "Apache-2.0",
        recommended_for: "celular / PC mais rápido",
    },
];

/// The model recommended for this platform.
pub const DEFAULT_ID: &str = if cfg!(target_os = "android") { "gemma-4-e2b" } else { "gemma-4-e4b" };

pub fn find(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}
