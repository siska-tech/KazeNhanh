mod common;

use kaze_nhanh::foundation::nlp::NlpService;

#[test]
fn real_dictionary_preserves_japanese_surfaces_and_byte_spans() {
    let service = NlpService::new(&common::nlp_config()).expect("real dictionary must load");
    let input = "東京都で自然な日本語を解析します。";
    let tokens = service.tokenize(input).expect("Japanese tokenization");
    assert!(tokens.len() > 3);
    assert!(tokens.iter().any(|token| !token.is_oov()));
    assert!(tokens
        .iter()
        .any(|token| token.part_of_speech().first().map(String::as_str) == Some("助詞")));
    let mut end = 0;
    for token in &tokens {
        assert_eq!(token.begin(), end);
        assert_eq!(&input[token.begin()..token.end()], token.surface());
        assert!(!token.part_of_speech().is_empty());
        assert_eq!(input[..token.begin()].chars().count(), token.begin_c());
        assert_eq!(input[..token.end()].chars().count(), token.end_c());
        end = token.end();
    }
    assert_eq!(end, input.len());
}

#[test]
fn real_dictionary_can_handle_unknown_characters() {
    let service = NlpService::new(&common::nlp_config()).expect("real dictionary must load");
    let input = "未登録XYZ123🌀";
    let tokens = service.tokenize(input).expect("OOV plugin must work");
    assert!(!tokens.is_empty());
    assert!(tokens.iter().any(|token| token.is_oov()));
    for token in &tokens {
        assert_eq!(&input[token.begin()..token.end()], token.surface());
    }
}

#[cfg(not(feature = "mock_inference"))]
#[test]
fn production_engine_rejects_non_gguf_model() {
    // If this target accidentally links the mock runtime, construction succeeds.
    let result = kaze_nhanh::KazeNhanhEngine::new(common::nlp_config());
    assert!(matches!(
        result,
        Err(kaze_nhanh::KazeNhanhError::ModelLoadError { .. })
    ));
}
