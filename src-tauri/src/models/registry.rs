use serde::{Deserialize, Serialize};

/// Available Whisper model sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-tiny.bin"
            }
            ModelSize::Base => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.bin"
            }
            ModelSize::Small => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-small.bin"
            }
            ModelSize::Medium => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-medium.bin"
            }
        }
    }

    pub fn sha256(&self) -> &'static str {
        match self {
            ModelSize::Tiny => "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
            ModelSize::Base => "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
            ModelSize::Small => "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
            ModelSize::Medium => "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
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

    /// All tiers ordered smallest/fastest to largest/highest-quality — the
    /// order #144 Phase 2 calibration walks when downgrading a tier that
    /// can't keep up with live speech on the current machine.
    pub const ALL_BY_QUALITY: [ModelSize; 4] = [
        ModelSize::Tiny,
        ModelSize::Base,
        ModelSize::Small,
        ModelSize::Medium,
    ];

    /// This tier's position in [`ModelSize::ALL_BY_QUALITY`] (`Tiny` = 0,
    /// `Medium` = 3) — used to find the next smaller tier during
    /// calibration without hardcoding the ordering a second time.
    fn quality_rank(&self) -> usize {
        Self::ALL_BY_QUALITY
            .iter()
            .position(|s| s == self)
            .expect("ALL_BY_QUALITY must list every ModelSize variant")
    }

    /// The largest tier in `downloaded` that is strictly smaller (lower
    /// quality/cost) than `self`, or `None` if `self` is already the
    /// smallest downloaded tier (or `downloaded` is empty / contains
    /// nothing smaller). Calibration (#144 Phase 2) uses this to step down
    /// one tier at a time when the current tier can't keep up live.
    pub fn next_smaller_downloaded(&self, downloaded: &[ModelSize]) -> Option<ModelSize> {
        let my_rank = self.quality_rank();
        downloaded
            .iter()
            .filter(|s| s.quality_rank() < my_rank)
            .max_by_key(|s| s.quality_rank())
            .copied()
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
                "https://raw.githubusercontent.com/snakers4/silero-vad/76e3dc408eb2a5c655c34e230d2d5459b4439daa/src/silero_vad/data/silero_vad.onnx"
            }
        }
    }

    pub fn sha256(&self) -> &'static str {
        match self {
            VadModel::Silero => "1a153a22f4509e292a94e67d6f9b85e8deb25b4988682b7e174c65279d8788e3",
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

    /// Pinned to an immutable commit SHA rather than a mutable branch/PR ref,
    /// so the bytes downloaded today are the bytes downloaded next year — see
    /// issue #54.
    pub fn revision(&self) -> &'static str {
        match self {
            TranslationModel::EnEs => "fdaddf76f50fcc1583ba42f95965862a7ab30f97",
            TranslationModel::EsEn => "725b7965a8cac11ebe80ea671e72e0b7e8b28a9f",
            TranslationModel::EnPt => "9f2863d807ecf91a374bdbecb8d01e402e90622e",
            TranslationModel::RomanceEn => "ddfee805aaa57f4bd198f88e8832ba2b012f9ae2",
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

    /// Expected SHA256 of `file` at this model's pinned `revision()`, for
    /// verifying downloaded bytes before they're fed to candle's mmapped
    /// safetensors parser — see issue #54.
    pub fn sha256(&self, file: &str) -> &'static str {
        match (self, file) {
            (TranslationModel::EnEs, "config.json") => {
                "666ad3c2943674b6a789e311d221e44ec23e27f1bd9727930754bf773ec8e464"
            }
            (TranslationModel::EnEs, "vocab.json") => {
                "257f346d7a6b2ecceafcca8ba05648ce2fd68dfaf105fb0e913dca7198f3f6d5"
            }
            (TranslationModel::EnEs, "source.spm") => {
                "4dd547c24816a335e7b0b2e63376a8f1b3cbfc671eda5ab808dd44fdadaa8791"
            }
            (TranslationModel::EnEs, "model.safetensors") => {
                "b3ecbf954573c2fd95d05d3ad4618baf961793db0da07c2add2e8a3a6cd78d0b"
            }
            (TranslationModel::EsEn, "config.json") => {
                "374443eacf8986c21386503e2fcd52eae6952ac09c87fd36695f5c12259a4cd1"
            }
            (TranslationModel::EsEn, "vocab.json") => {
                "257f346d7a6b2ecceafcca8ba05648ce2fd68dfaf105fb0e913dca7198f3f6d5"
            }
            (TranslationModel::EsEn, "source.spm") => {
                "e236ee6d866b635c0142114f8647f39831f9d92534aa2aad75c942f6a78ad0e3"
            }
            (TranslationModel::EsEn, "model.safetensors") => {
                "07d9fc8881ac9bc8f06fbe3576ca16045c684c7d529e9733cbeeaaf2c78f9539"
            }
            (TranslationModel::EnPt, "config.json") => {
                "ca76b1818f066007e94fb2519c0752320cee36a5d4947bf7ef4477c845feacc5"
            }
            (TranslationModel::EnPt, "vocab.json") => {
                "dad10ad0acbf34ad92af16cb37fd71732d2b73851274698d58c5439386b506a1"
            }
            (TranslationModel::EnPt, "source.spm") => {
                "7a7fcf812cf03a5785daa35d4932bbbe69e7e605c0fe56fce5a3f731d6c355aa"
            }
            (TranslationModel::EnPt, "model.safetensors") => {
                "f1772ec97f6cb5b942bb6a5555a04272960a228a523f7ed47e24014236aa1716"
            }
            (TranslationModel::RomanceEn, "config.json") => {
                "b11b54220c28a64966b51864dd4bf9688a935c3b6a18bbab73810d391d6ac39f"
            }
            (TranslationModel::RomanceEn, "vocab.json") => {
                "1ffaf15a0b51f0774c1dc24a1a675859dc18fc7705ebe6a0ac45a9c560457c29"
            }
            (TranslationModel::RomanceEn, "source.spm") => {
                "baeab21ec00d0b490382b82499ef0348235c5a8e75de28aec8290adf62b007c4"
            }
            (TranslationModel::RomanceEn, "model.safetensors") => {
                "4933c13d13c01bc2f59e36d3419a7c44b3696b345b658a2eef5d1f3d6195b6b4"
            }
            _ => {
                unreachable!("sha256 requested for a file outside TRANSLATION_MODEL_FILES: {file}")
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn is_sha256_hex(s: &str) -> bool {
        s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
    }

    #[test]
    fn test_model_size_sha256_valid() {
        let sizes = [
            ModelSize::Tiny,
            ModelSize::Base,
            ModelSize::Small,
            ModelSize::Medium,
        ];

        for size in sizes {
            let sha = size.sha256();
            assert!(is_sha256_hex(sha), "Invalid SHA256 for {:?}: {}", size, sha);
        }
    }

    #[test]
    fn test_model_size_download_url_non_empty() {
        let sizes = [
            ModelSize::Tiny,
            ModelSize::Base,
            ModelSize::Small,
            ModelSize::Medium,
        ];

        for size in sizes {
            let url = size.download_url();
            assert!(!url.is_empty(), "Empty download URL for {:?}", size);
            assert!(
                url.starts_with("https://"),
                "Invalid URL scheme for {:?}",
                size
            );
        }
    }

    #[test]
    fn test_vad_model_sha256_valid() {
        let sha = VadModel::Silero.sha256();
        assert!(is_sha256_hex(sha), "Invalid SHA256 for Silero: {}", sha);
    }

    #[test]
    fn test_vad_model_download_url_non_empty() {
        let url = VadModel::Silero.download_url();
        assert!(!url.is_empty(), "Empty download URL for Silero VAD");
        assert!(
            url.starts_with("https://"),
            "Invalid URL scheme for Silero VAD"
        );
    }

    #[test]
    fn test_translation_model_sha256_valid() {
        let models = [
            TranslationModel::EnEs,
            TranslationModel::EsEn,
            TranslationModel::EnPt,
            TranslationModel::RomanceEn,
        ];

        for model in models {
            for file in TRANSLATION_MODEL_FILES {
                let sha = model.sha256(file);
                assert!(
                    is_sha256_hex(sha),
                    "Invalid SHA256 for {:?} file {}: {}",
                    model,
                    file,
                    sha
                );
            }
        }
    }

    #[test]
    fn test_translation_model_sha256_covers_all_files() {
        let models = [
            TranslationModel::EnEs,
            TranslationModel::EsEn,
            TranslationModel::EnPt,
            TranslationModel::RomanceEn,
        ];

        for model in models {
            for file in TRANSLATION_MODEL_FILES {
                let sha = model.sha256(file);
                assert!(
                    !sha.is_empty(),
                    "Empty SHA256 for {:?} file {}",
                    model,
                    file
                );
            }
        }
    }

    #[test]
    fn test_quality_rank_is_strictly_increasing_by_quality() {
        assert!(ModelSize::Tiny.quality_rank() < ModelSize::Base.quality_rank());
        assert!(ModelSize::Base.quality_rank() < ModelSize::Small.quality_rank());
        assert!(ModelSize::Small.quality_rank() < ModelSize::Medium.quality_rank());
    }

    #[test]
    fn test_next_smaller_downloaded_finds_largest_option_below_self() {
        let downloaded = [ModelSize::Tiny, ModelSize::Base, ModelSize::Medium];
        assert_eq!(
            ModelSize::Medium.next_smaller_downloaded(&downloaded),
            Some(ModelSize::Base)
        );
    }

    #[test]
    fn test_next_smaller_downloaded_none_when_self_is_smallest_downloaded() {
        let downloaded = [ModelSize::Tiny, ModelSize::Small];
        assert_eq!(ModelSize::Tiny.next_smaller_downloaded(&downloaded), None);
    }

    #[test]
    fn test_next_smaller_downloaded_none_when_downloaded_is_empty() {
        assert_eq!(ModelSize::Medium.next_smaller_downloaded(&[]), None);
    }

    #[test]
    fn test_next_smaller_downloaded_ignores_tiers_larger_than_self() {
        // Only Medium is downloaded, which is larger than Small — nothing
        // smaller than Small is available, so this must be None, not Medium.
        let downloaded = [ModelSize::Medium];
        assert_eq!(ModelSize::Small.next_smaller_downloaded(&downloaded), None);
    }
}
