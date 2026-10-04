use std::path::PathBuf;
use std::sync::OnceLock;

use kaze_nhanh::EngineConfig;

// Only tests keep this allocation for the process lifetime, as required by the
// existing static EngineConfig API. Production resource ownership is P1 work.
pub fn nlp_config() -> EngineConfig {
    static DICTIONARY: OnceLock<Vec<u8>> = OnceLock::new();
    let dictionary = DICTIONARY.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/sudachi/system.dic");
        std::fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "Cannot read Sudachi dictionary at {}: {error}. Run scripts/dev/setup-dev.ps1 first.",
                path.display()
            )
        })
    });
    EngineConfig::new(
        b"mock-model",
        dictionary.as_slice(),
        include_bytes!("../../resources/sudachi/sudachi.json"),
    )
}
