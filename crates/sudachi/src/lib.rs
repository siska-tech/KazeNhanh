//! Owned local Sudachi resources and backend-neutral morphology.
use kaze_nhanh_core::{
    ArtifactIdentity, ByteSpan, EvaluationError, MorphAnalysis, MorphAnalyzer, Morpheme,
};
use sha2::{Digest, Sha256};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
    sync::Arc,
};
use sudachi::{
    analysis::{stateless_tokenizer::StatelessTokenizer, Mode, Tokenize},
    config::ConfigBuilder,
    dic::{
        dictionary::JapaneseDictionary,
        storage::{Storage, SudachiDicData},
    },
};

#[derive(Clone, Copy, Debug, Default)]
pub enum SudachiMode {
    A,
    B,
    #[default]
    C,
}
/// Owns all resources; no leaked or static input buffers are required.
pub struct SudachiConfig {
    pub dictionary: Vec<u8>,
    pub settings: Vec<u8>,
    pub mode: SudachiMode,
}
impl SudachiConfig {
    pub fn from_paths(
        dictionary: impl AsRef<Path>,
        settings: impl AsRef<Path>,
        mode: SudachiMode,
    ) -> Result<Self, EvaluationError> {
        let load = |path: &Path| {
            std::fs::read(path).map_err(|err| EvaluationError::Backend {
                backend: "sudachi.assets".into(),
                message: format!("{}: {err}", path.display()),
            })
        };
        Ok(Self {
            dictionary: load(dictionary.as_ref())?,
            settings: load(settings.as_ref())?,
            mode,
        })
    }
}
#[derive(Clone)]
pub struct SudachiAnalyzer {
    tokenizer: Arc<StatelessTokenizer<Arc<JapaneseDictionary>>>,
    mode: SudachiMode,
    provenance: Vec<ArtifactIdentity>,
}
fn backend_error(error: impl std::fmt::Display) -> EvaluationError {
    EvaluationError::Backend {
        backend: "sudachi".into(),
        message: error.to_string(),
    }
}
impl SudachiAnalyzer {
    pub fn new(config: SudachiConfig) -> Result<Self, EvaluationError> {
        let provenance = vec![
            ArtifactIdentity {
                component: "dictionary".into(),
                id: "sudachi.system".into(),
                sha256: Some(format!("{:x}", Sha256::digest(&config.dictionary))),
            },
            ArtifactIdentity {
                component: "settings".into(),
                id: "sudachi.settings".into(),
                sha256: Some(format!("{:x}", Sha256::digest(&config.settings))),
            },
            ArtifactIdentity {
                component: "analyzer".into(),
                id: format!("sudachi.rs.v0.6.9.mode.{:?}", config.mode),
                sha256: None,
            },
        ];
        let settings = ConfigBuilder::from_bytes(&config.settings)
            .map_err(backend_error)?
            .build();
        let storage = SudachiDicData::new(Storage::Owned(config.dictionary));
        let dictionary = catch_unwind(AssertUnwindSafe(|| {
            JapaneseDictionary::from_cfg_storage_with_embedded_chardef(&settings, storage)
        }))
        .map_err(|_| backend_error("invalid dictionary grammar"))?
        .map_err(backend_error)?;
        Ok(Self {
            tokenizer: Arc::new(StatelessTokenizer::new(Arc::new(dictionary))),
            mode: config.mode,
            provenance,
        })
    }
}
impl MorphAnalyzer for SudachiAnalyzer {
    fn analyze(&self, text: &str) -> Result<MorphAnalysis, EvaluationError> {
        let mode = match self.mode {
            SudachiMode::A => Mode::A,
            SudachiMode::B => Mode::B,
            SudachiMode::C => Mode::C,
        };
        let list = self
            .tokenizer
            .tokenize(text, mode, false)
            .map_err(backend_error)?;
        let mut morphemes = Vec::with_capacity(list.len());
        for item in list.iter() {
            morphemes.push(Morpheme {
                span: ByteSpan::new(text, item.begin(), item.end())?,
                surface: item.surface().to_string(),
                dictionary_form: item.dictionary_form().into(),
                normalized_form: item.normalized_form().into(),
                reading: item.reading_form().into(),
                part_of_speech: item.part_of_speech().to_vec(),
                is_oov: item.is_oov(),
                dictionary_id: item.dictionary_id(),
                synonym_group_ids: item.synonym_group_ids().to_vec(),
                cumulative_cost: Some(item.total_cost()),
            });
        }
        Ok(MorphAnalysis {
            morphemes,
            provenance: self.provenance.clone(),
        })
    }
}
