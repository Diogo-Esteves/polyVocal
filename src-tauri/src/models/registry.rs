#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Available Whisper model sizes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelSize {
    Tiny,
    Base,
    Small,
    Medium,
}

impl ModelSize {
    pub fn filename(&self) -> &'static str {
        match self {
            ModelSize::Tiny => "ggml-tiny.bin",
            ModelSize::Base => "ggml-base.bin",
            ModelSize::Small => "ggml-small.bin",
            ModelSize::Medium => "ggml-medium.bin",
        }
    }

    pub fn download_url(&self) -> &'static str {
        // Hugging Face — ggerganov/whisper.cpp model files
        match self {
            ModelSize::Tiny => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin"
            }
            ModelSize::Base => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
            }
            ModelSize::Small => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
            }
            ModelSize::Medium => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin"
            }
        }
    }

    pub fn size_mb(&self) -> u32 {
        match self {
            ModelSize::Tiny => 75,
            ModelSize::Base => 145,
            ModelSize::Small => 465,
            ModelSize::Medium => 1500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub size: ModelSize,
    pub downloaded: bool,
    pub is_active: bool,
}
