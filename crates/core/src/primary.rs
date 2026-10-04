//! Explainable Japanese primary screening, not a grammar/semantic oracle.
use crate::*;

pub const PRIMARY_RULES_ID: &str = "kzn.primary_rules.v1";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextFormat {
    FreeText,
    AsciiDigits,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainProfile {
    pub id: String,
    pub required: bool,
    /// Unicode scalar count, independent of the API's byte resource limit.
    pub max_chars: Option<usize>,
    pub format: TextFormat,
    pub forbid_controls: bool,
    pub screen_naturalness: bool,
    pub repeated_char_min: Option<usize>,
    pub repeated_token_min: Option<usize>,
    pub allowed_terms: Vec<String>,
}
impl DomainProfile {
    pub fn screening() -> Self {
        Self {
            id: "ja.primary.v1".into(),
            required: false,
            max_chars: None,
            format: TextFormat::FreeText,
            forbid_controls: true,
            screen_naturalness: true,
            repeated_char_min: Some(6),
            repeated_token_min: Some(3),
            allowed_terms: vec![],
        }
    }
    pub fn builtin(id: &str) -> Result<Self, EvaluationError> {
        let mut profile = Self::screening();
        match id {
            "ja.primary.v1" | "ja.ocr.v1" | "ja.asr.v1" | "ja.llm.v1" => {}
            "ja.form.v1" => {
                profile.required = true;
                profile.screen_naturalness = false;
                profile.repeated_char_min = None;
                profile.repeated_token_min = None;
            }
            _ => {
                return Err(EvaluationError::InvalidConfig(format!(
                    "unknown built-in profile: {id}; supply an explicit DomainProfile"
                )))
            }
        }
        profile.id = id.into();
        Ok(profile)
    }
    pub fn evaluation_config(&self) -> EvaluationConfig {
        let mut config = EvaluationConfig {
            profile_id: self.id.clone(),
            ..EvaluationConfig::default()
        };
        if !self.screen_naturalness {
            config.required_dimensions = vec![Dimension::Validity];
        }
        if self.id == "ja.llm.v1" {
            config.semantic_scope = ScoreScope::Reference;
            config
                .required_dimensions
                .push(Dimension::SemanticConsistency);
        }
        config
    }
    pub(crate) fn validate(&self) -> Result<(), EvaluationError> {
        if self.id.trim().is_empty()
            || self.max_chars == Some(0)
            || self.repeated_char_min.is_some_and(|v| v < 3)
            || self.repeated_token_min.is_some_and(|v| v < 2)
            || self.allowed_terms.len() > 1000
            || self
                .allowed_terms
                .iter()
                .any(|s| s.is_empty() || s.len() > 256)
        {
            return Err(EvaluationError::InvalidConfig(
                "invalid primary profile limits/allowlist".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MorphologyFeatures {
    pub token_count: usize,
    pub oov_count: usize,
    pub max_consecutive_oov: usize,
    pub normalized_difference_count: usize,
    pub particle_count: usize,
    pub auxiliary_count: usize,
}
impl MorphologyFeatures {
    pub fn extract(analysis: &MorphAnalysis) -> Self {
        let mut features = Self {
            token_count: analysis.morphemes.len(),
            ..Self::default()
        };
        let mut run = 0;
        let mut previous_end = None;
        for token in &analysis.morphemes {
            if token.is_oov {
                features.oov_count += 1;
                run = if previous_end == Some(token.span.start()) {
                    run + 1
                } else {
                    1
                };
                features.max_consecutive_oov = features.max_consecutive_oov.max(run);
            } else {
                run = 0;
            }
            features.normalized_difference_count +=
                usize::from(token.surface != token.normalized_form);
            features.particle_count +=
                usize::from(token.part_of_speech.first().map(String::as_str) == Some("助詞"));
            features.auxiliary_count +=
                usize::from(token.part_of_speech.first().map(String::as_str) == Some("助動詞"));
            previous_end = Some(token.span.end());
        }
        features
    }
}
#[derive(Clone)]
pub struct PrimaryRules {
    profile: DomainProfile,
}
impl PrimaryRules {
    pub fn new(profile: DomainProfile) -> Result<Self, EvaluationError> {
        profile.validate()?;
        Ok(Self { profile })
    }
}
fn issue(
    text: &str,
    start: usize,
    end: usize,
    code: &str,
    severity: Severity,
    evidence: serde_json::Value,
    explanation: &str,
) -> Issue {
    Issue {
        code: code.into(),
        severity,
        span: ByteSpan::new(text, start, end)
            .expect("rule offsets come from validated UTF-8 boundaries"),
        stage: DetectionStage::Primary,
        evidence,
        explanation: explanation.into(),
    }
}
fn score(value: f32, scope: ScoreScope) -> DimensionScore {
    DimensionScore {
        value: Some(UnitScore::new(value).expect("bounded heuristic score")),
        status: ScoreStatus::Evaluated,
        method: Some(ScoreMethod::Heuristic),
        scope,
        calibration_id: None,
        confidence: None,
    }
}
fn japanese_letter(c: char) -> bool {
    matches!(c, '\u{3041}'..='\u{3096}' | '\u{30a1}'..='\u{30fa}' | '\u{4e00}'..='\u{9fff}')
}
impl PrimaryDetector for PrimaryRules {
    fn profile(&self) -> Option<DomainProfile> {
        Some(self.profile.clone())
    }
    fn artifacts(&self) -> Vec<ArtifactIdentity> {
        vec![
            ArtifactIdentity {
                component: "rules".into(),
                id: PRIMARY_RULES_ID.into(),
                sha256: None,
            },
            ArtifactIdentity {
                component: "profile".into(),
                id: self.profile.id.clone(),
                sha256: None,
            },
        ]
    }
    fn limitations(&self) -> Vec<String> {
        vec![
            "naturalness_screening_only_not_grammar_proof".into(),
            "semantic_consistency_not_assessed".into(),
            "heuristic_scores_not_calibrated_probabilities".into(),
            "oov_and_normalization_are_features_not_errors".into(),
        ]
    }
    fn detect(
        &self,
        input: &TextInput<'_>,
        analysis: &MorphAnalysis,
        config: &EvaluationConfig,
    ) -> Result<PrimaryEvaluation, EvaluationError> {
        if config.profile_id != self.profile.id {
            return Err(EvaluationError::InvalidConfig(
                "detector/profile identifier mismatch".into(),
            ));
        }
        let text = input.text;
        let mut issues = Findings::default();
        if self.profile.required && text.trim().is_empty() {
            issues.push(issue(
                text,
                0,
                text.len(),
                "required_empty",
                Severity::Error,
                serde_json::json!({"required":true}),
                "必須入力が空です。",
            ));
        }
        let char_count = text.chars().count();
        if self.profile.max_chars.is_some_and(|max| char_count > max) {
            issues.push(issue(
                text,
                0,
                text.len(),
                "max_chars_exceeded",
                Severity::Error,
                serde_json::json!({"actual":char_count,"limit":self.profile.max_chars}),
                "profileの文字数上限を超えています。",
            ));
        }
        if self.profile.format == TextFormat::AsciiDigits
            && !text.trim().is_empty()
            && !text.trim().bytes().all(|b| b.is_ascii_digit())
        {
            issues.push(issue(
                text,
                0,
                text.len(),
                "format_mismatch",
                Severity::Error,
                serde_json::json!({"expected":"ascii_digits","trim_outer_whitespace":true}),
                "ASCII数字のみを許可する入力制約に一致しません。",
            ));
        }
        let mut brackets: Vec<(char, usize)> = Vec::new();
        for (offset, ch) in text.char_indices() {
            let end = offset + ch.len_utf8();
            if self.profile.forbid_controls && ch.is_control() && !matches!(ch, '\n' | '\r' | '\t')
            {
                issues.push(issue(
                    text,
                    offset,
                    end,
                    "forbidden_control",
                    Severity::Error,
                    serde_json::json!({"unicode":format!("U+{:04X}",ch as u32)}),
                    "profileが許可しない制御文字です。",
                ));
            }
            if self.profile.screen_naturalness && ch == '\u{fffd}' {
                issues.push(issue(
                    text,
                    offset,
                    end,
                    "replacement_character",
                    Severity::Warning,
                    serde_json::json!({"unicode":"U+FFFD"}),
                    "文字化け・欠落の候補です。意図的な使用の可能性もあります。",
                ));
            }
            if !self.profile.screen_naturalness {
                continue;
            }
            let closer = match ch {
                '(' => Some(')'),
                '（' => Some('）'),
                '[' => Some(']'),
                '［' => Some('］'),
                '{' => Some('}'),
                '「' => Some('」'),
                '『' => Some('』'),
                '【' => Some('】'),
                _ => None,
            };
            if let Some(closer) = closer {
                brackets.push((closer, offset));
            } else if matches!(ch, ')' | '）' | ']' | '］' | '}' | '」' | '』' | '】') {
                if brackets.last().is_some_and(|(expected, _)| *expected == ch) {
                    brackets.pop();
                } else {
                    issues.push(issue(
                        text,
                        offset,
                        end,
                        "unmatched_bracket",
                        Severity::Warning,
                        serde_json::json!({"kind":"closing","actual":ch.to_string()}),
                        "対応する括弧が確認できません。引用や断片入力の可能性があります。",
                    ));
                }
            }
        }
        for (expected, offset) in brackets {
            let ch = text[offset..].chars().next().unwrap();
            issues.push(issue(
                text,
                offset,
                offset + ch.len_utf8(),
                "unmatched_bracket",
                Severity::Warning,
                serde_json::json!({"kind":"opening","expected":expected.to_string()}),
                "閉じ括弧が確認できません。断片入力の可能性があります。",
            ));
        }
        if let Some(min) = self
            .profile
            .repeated_char_min
            .filter(|_| self.profile.screen_naturalness)
        {
            let mut run: Option<(char, usize, usize, usize)> = None;
            for (offset, ch) in text
                .char_indices()
                .chain(std::iter::once((text.len(), '\0')))
            {
                if let Some((previous, start, count, end)) = run {
                    if previous == ch {
                        run = Some((ch, start, count + 1, offset + ch.len_utf8()));
                        continue;
                    }
                    if count >= min
                        && japanese_letter(previous)
                        && !self
                            .profile
                            .allowed_terms
                            .iter()
                            .any(|term| term == &text[start..end])
                    {
                        issues.push(issue(
                            text,
                            start,
                            end,
                            "repeated_character",
                            Severity::Warning,
                            serde_json::json!({"count":count,"threshold":min}),
                            "日本語文字の連続反復です。意図的な表現か確認が必要です。",
                        ));
                    }
                }
                run = Some((ch, offset, 1, offset + ch.len_utf8()));
            }
        }
        let character_spans = issues
            .items
            .iter()
            .filter(|i| i.code == "repeated_character")
            .map(|i| i.span)
            .collect::<Vec<_>>();
        if let Some(min) = self
            .profile
            .repeated_token_min
            .filter(|_| self.profile.screen_naturalness)
        {
            let tokens = &analysis.morphemes;
            let mut start = 0;
            while start < tokens.len() {
                let mut end = start + 1;
                while end < tokens.len()
                    && tokens[end].surface == tokens[start].surface
                    && tokens[end - 1].span.end() == tokens[end].span.start()
                {
                    end += 1;
                }
                let token = &tokens[start];
                if end - start >= min
                    && token.surface.chars().any(japanese_letter)
                    && !self.profile.allowed_terms.contains(&token.surface)
                {
                    let span_start = token.span.start();
                    let span_end = tokens[end - 1].span.end();
                    if self
                        .profile
                        .allowed_terms
                        .iter()
                        .any(|term| term == &text[span_start..span_end])
                    {
                        start = end;
                        continue;
                    }
                    // Character repetition already explains the same range.
                    if !character_spans
                        .get(
                            character_spans
                                .partition_point(|span| span.start() <= span_start)
                                .wrapping_sub(1),
                        )
                        .is_some_and(|span| span.end() >= span_end)
                    {
                        issues.push(issue(text,span_start,span_end,"repeated_token",Severity::Warning,serde_json::json!({"count":end-start,"threshold":min,"surface":token.surface}),"同じ形態素が連続しています。認識重複や意図的反復の候補です。"));
                    }
                }
                start = end;
            }
        }
        let issues_error_count = issues.errors;
        let invalid = issues_error_count > 0;
        let warnings = issues.warnings;
        let total = issues.total;
        let mut issues = issues.items;
        issues.sort_by_key(|i| (i.span.start(), i.span.end(), i.code.clone()));
        if total > issues.len() {
            let retained = issues.len();
            issues.push(issue(
                text,
                0,
                text.len(),
                "findings_truncated",
                Severity::Info,
                serde_json::json!({"total":total,"retained":retained,"omitted":total-retained,"errors":issues_error_count,"warnings":warnings}),
                "全入力を検査しましたが、根拠の出力件数を制限しています。",
            ));
        }
        Ok(PrimaryEvaluation {
            verdict: if invalid {
                Verdict::Invalid
            } else if warnings > 0 {
                Verdict::Suspicious
            } else {
                Verdict::Acceptable
            },
            scores: Scores {
                validity: score(if invalid { 0.0 } else { 1.0 }, ScoreScope::FullText),
                naturalness: if !self.profile.screen_naturalness || text.trim().is_empty() {
                    DimensionScore::unassessed(ScoreScope::FullText, ScoreStatus::NotApplicable)
                } else {
                    score((1.0 - 0.2 * warnings as f32).max(0.0), ScoreScope::FullText)
                },
                semantic_consistency: DimensionScore::unassessed(
                    config.semantic_scope,
                    ScoreStatus::NotEvaluated,
                ),
            },
            issues,
        })
    }
}

/// Bound output memory while still scanning every input character/token.
#[derive(Default)]
struct Findings {
    items: Vec<Issue>,
    errors: usize,
    warnings: usize,
    total: usize,
}
impl Findings {
    fn push(&mut self, issue: Issue) {
        self.total += 1;
        self.errors += usize::from(issue.severity == Severity::Error);
        self.warnings += usize::from(issue.severity == Severity::Warning);
        if self.items.len() < 256 {
            self.items.push(issue);
        } else if issue.severity == Severity::Error && self.errors == 1 {
            self.items.pop();
            self.items.push(issue);
        }
    }
}
