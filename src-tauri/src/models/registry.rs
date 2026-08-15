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

/// Available VAD models. Silero is the only one today — no size variants,
/// since VAD is a fixed pipeline component rather than a user-selectable
/// quality/speed tradeoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VadModel {
    Silero,
}

impl VadModel {
    pub fn filename(&self) -> &'static str {
        match self {
            VadModel::Silero => "silero_vad.onnx",
        }
    }

    pub fn download_url(&self) -> &'static str {
        match self {
            VadModel::Silero => {
                "https://raw.githubusercontent.com/snakers4/silero-vad/master/src/silero_vad/data/silero_vad.onnx"
            }
        }
    }
}

/// The individual OPUS-MT (MarianMT) checkpoints backing translation, per
/// DEC-010. There is no single Helsinki-NLP model for every MVP pair — in
/// particular pt<->es has no direct model — so [`translation::engine`] chains
/// these underlying models rather than each `TranslationModel` mapping 1:1
/// to a language pair. See DEC-010's consequences for the full pair→model(s)
/// mapping.
///
/// [`translation::engine`]: crate::translation::engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationModel {
    /// Helsinki-NLP/opus-mt-en-es — English -> Spanish.
    EnEs,
    /// Helsinki-NLP/opus-mt-es-en — Spanish -> English.
    EsEn,
    /// Helsinki-NLP/opus-mt-tc-big-en-pt — English -> Portuguese. A
    /// multi-target model: requires the `>>por<<` prefix (see
    /// [`TranslationModel::target_prefix`]) to select European/standard
    /// Portuguese over Brazilian Portuguese (`>>pob<<`).
    EnPt,
    /// Helsinki-NLP/opus-mt-ROMANCE-en — many Romance languages
    /// (including pt and es) -> English. Used as the pt->en leg, since no
    /// dedicated Helsinki-NLP opus-mt-pt-en model exists.
    RomanceEn,
}

/// Files pulled from each model's HuggingFace repo. `model.safetensors`
/// rather than `pytorch_model.bin`: several of these repos' `main` branch
/// only ships the legacy pre-1.6 PyTorch pickle format (no zip container),
/// which candle's pickle reader doesn't support (it expects the modern
/// zip-based format) — see each model's [`TranslationModel::revision`].
pub const TRANSLATION_MODEL_FILES: [&str; 4] = [
    "config.json",
    "vocab.json",
    "source.spm",
    "model.safetensors",
];

impl TranslationModel {
    /// HuggingFace repo id this model's files are downloaded from.
    pub fn hf_repo(&self) -> &'static str {
        match self {
            TranslationModel::EnEs => "Helsinki-NLP/opus-mt-en-es",
            TranslationModel::EsEn => "Helsinki-NLP/opus-mt-es-en",
            TranslationModel::EnPt => "Helsinki-NLP/opus-mt-tc-big-en-pt",
            TranslationModel::RomanceEn => "Helsinki-NLP/opus-mt-ROMANCE-en",
        }
    }

    /// Revision/ref to download files from. Defaults to `main`, except
    /// where `main` has no `model.safetensors` yet — HuggingFace's
    /// auto-conversion bot opens (and, for these repos, leaves unmerged) a
    /// PR adding one, so we pull `model.safetensors` from that PR's ref
    /// instead. `config.json`/`vocab.json`/`source.spm` are identical
    /// between `main` and these PRs (only the weight format differs), so
    /// this is safe to apply to every file, not just the weights.
    pub fn revision(&self) -> &'static str {
        match self {
            TranslationModel::EnEs => "refs/pr/4",
            TranslationModel::EsEn => "refs/pr/6",
            TranslationModel::EnPt => "main",
            TranslationModel::RomanceEn => "refs/pr/6",
        }
    }

    /// Subdirectory name under the models dir's `translation/` folder.
    pub fn dir_name(&self) -> &'static str {
        match self {
            TranslationModel::EnEs => "en-es",
            TranslationModel::EsEn => "es-en",
            TranslationModel::EnPt => "en-pt",
            TranslationModel::RomanceEn => "romance-en",
        }
    }

    /// Download URL for one of [`TRANSLATION_MODEL_FILES`]. The revision
    /// segment is percent-encoded (`refs/pr/4` -> `refs%2Fpr%2F4`) — HF's
    /// `resolve` endpoint 404s on a literal `/` there, since it would
    /// otherwise be ambiguous with a path inside the repo.
    pub fn download_url(&self, file: &str) -> String {
        let revision = self.revision().replace('/', "%2F");
        format!(
            "https://huggingface.co/{}/resolve/{revision}/{file}",
            self.hf_repo()
        )
    }

    /// Sentence-initial target-language token this model requires, if any.
    /// `EnPt` is trained multi-target (`pob`/`por`); we always ask for
    /// standard Portuguese. Bilingual and many-to-one models need nothing.
    pub fn target_prefix(&self) -> Option<&'static str> {
        match self {
            TranslationModel::EnPt => Some(">>por<< "),
            _ => None,
        }
    }

    /// Approximate on-disk size of this checkpoint, in decimal MB — Settings
    /// shows this next to the language pair it belongs to, per
    /// `../../design/DESIGN.md` principle 5 ("Languages, not files"), same
    /// idea as `ModelSize::size_mb` for Whisper. Dominated entirely by
    /// `model.safetensors` (the other `TRANSLATION_MODEL_FILES` are under
    /// 1 MB combined); measured from each repo's HuggingFace
    /// `Content-Length` at the pinned `revision()`, not computed at
    /// runtime — not worth a network round trip just to report a number in
    /// Settings.
    pub fn size_mb(&self) -> u32 {
        match self {
            TranslationModel::EnEs => 312,
            TranslationModel::EsEn => 312,
            TranslationModel::EnPt => 465,
            TranslationModel::RomanceEn => 312,
        }
    }
}
