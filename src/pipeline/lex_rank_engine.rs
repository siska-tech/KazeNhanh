use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use thiserror::Error;

use super::hybrid_summarizer::HybridSummarizerConfig;
use super::sentence::SentenceNode;

#[derive(Clone, Debug)]
pub(crate) struct LexRankOptions {
    pub similarity_threshold: f32,
    pub damping_factor: f32,
    pub max_iterations: usize,
    pub convergence_delta: f32,
}

impl From<&HybridSummarizerConfig> for LexRankOptions {
    fn from(config: &HybridSummarizerConfig) -> Self {
        Self {
            similarity_threshold: config.similarity_threshold,
            damping_factor: config.damping_factor,
            max_iterations: config.max_iterations,
            convergence_delta: config.convergence_delta,
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum LexRankError {
    #[error("lexrank failed to converge within {iterations} iterations (delta={delta})")]
    DidNotConverge { iterations: usize, delta: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RankedSentence {
    pub position: usize,
    pub score: f32,
    pub text: String,
}

impl RankedSentence {
    pub(crate) fn new(position: usize, score: f32, text: String) -> Self {
        Self {
            position,
            score,
            text,
        }
    }
}

pub(crate) fn rank_sentences(
    sentences: &[SentenceNode],
    options: &LexRankOptions,
) -> Result<Vec<RankedSentence>, LexRankError> {
    if sentences.is_empty() {
        return Ok(Vec::new());
    }

    let tfidf_vectors = build_tfidf_vectors(sentences);
    let mut similarity_matrix =
        build_similarity_matrix(&tfidf_vectors, options.similarity_threshold);

    normalize_rows(&mut similarity_matrix);

    let mut scores = vec![1.0 / sentences.len() as f32; sentences.len()];
    let mut converged = false;

    for iteration in 0..options.max_iterations {
        let updated_scores = iterate_pagerank(&scores, &similarity_matrix, options.damping_factor);
        let normalized = normalize_scores(updated_scores);
        let delta = l1_delta(&scores, &normalized);

        scores = normalized;
        if delta < options.convergence_delta {
            converged = true;
            break;
        }

        if iteration + 1 == options.max_iterations {
            return Err(LexRankError::DidNotConverge {
                iterations: options.max_iterations,
                delta,
            });
        }
    }

    if !converged {
        // Should not reach here because loop returns Err on final iteration
        return Err(LexRankError::DidNotConverge {
            iterations: options.max_iterations,
            delta: options.convergence_delta,
        });
    }

    let mut ranked = sentences
        .iter()
        .zip(scores.iter())
        .map(|(sentence, score)| {
            RankedSentence::new(sentence.position, *score, sentence.text.clone())
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|a, b| match b.score.partial_cmp(&a.score) {
        Some(Ordering::Equal) | None => a.position.cmp(&b.position),
        Some(ordering) => ordering,
    });

    Ok(ranked)
}

fn build_tfidf_vectors(sentences: &[SentenceNode]) -> Vec<HashMap<String, f32>> {
    let mut document_frequency: HashMap<&str, usize> = HashMap::new();

    for sentence in sentences {
        let mut seen = HashSet::new();
        for token in &sentence.tokens {
            if seen.insert(token.as_str()) {
                *document_frequency.entry(token.as_str()).or_insert(0) += 1;
            }
        }
    }

    let doc_count = sentences.len() as f32;

    let mut vectors = Vec::with_capacity(sentences.len());
    for sentence in sentences {
        let total_tokens = sentence.tokens.len() as f32;
        let mut vector = HashMap::with_capacity(sentence.bag_of_words.len());

        for (token, frequency) in &sentence.bag_of_words {
            if let Some(df) = document_frequency.get(token.as_str()) {
                let idf = ((doc_count + 1.0) / (*df as f32 + 1.0)).ln() + 1.0;
                let tf = frequency / total_tokens.max(1.0);
                vector.insert(token.clone(), tf * idf);
            }
        }

        vectors.push(vector);
    }

    vectors
}

fn build_similarity_matrix(vectors: &[HashMap<String, f32>], threshold: f32) -> Vec<Vec<f32>> {
    let count = vectors.len();
    let mut matrix = vec![vec![0.0; count]; count];

    for i in 0..count {
        matrix[i][i] = 0.0;
        for j in (i + 1)..count {
            let similarity = cosine_similarity(&vectors[i], &vectors[j]);
            if similarity >= threshold {
                matrix[i][j] = similarity;
                matrix[j][i] = similarity;
            }
        }
    }

    matrix
}

fn cosine_similarity(a: &HashMap<String, f32>, b: &HashMap<String, f32>) -> f32 {
    let (smaller, larger) = if a.len() <= b.len() { (a, b) } else { (b, a) };

    let mut dot_product = 0.0;
    for (token, weight) in smaller {
        if let Some(other) = larger.get(token) {
            dot_product += weight * other;
        }
    }

    let norm_a = vector_norm(a);
    let norm_b = vector_norm(b);

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

fn vector_norm(vector: &HashMap<String, f32>) -> f32 {
    vector
        .values()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt()
}

fn normalize_rows(matrix: &mut [Vec<f32>]) {
    let count = matrix.len();
    if count == 0 {
        return;
    }

    let uniform = 1.0 / count as f32;

    for row in matrix.iter_mut() {
        let sum: f32 = row.iter().sum();
        if sum.abs() < f32::EPSILON {
            for value in row.iter_mut() {
                *value = uniform;
            }
        } else {
            for value in row.iter_mut() {
                *value /= sum;
            }
        }
    }
}

fn iterate_pagerank(scores: &[f32], matrix: &[Vec<f32>], damping_factor: f32) -> Vec<f32> {
    let count = scores.len();
    let teleport = (1.0 - damping_factor) / count as f32;
    let mut updated = vec![0.0; count];

    for (i, row) in matrix.iter().enumerate() {
        for (j, weight) in row.iter().enumerate() {
            updated[j] += damping_factor * scores[i] * weight;
        }
    }

    for value in updated.iter_mut() {
        *value += teleport;
    }

    updated
}

fn normalize_scores(scores: Vec<f32>) -> Vec<f32> {
    let sum: f32 = scores.iter().sum();
    if sum.abs() < f32::EPSILON {
        let uniform = 1.0 / scores.len() as f32;
        return vec![uniform; scores.len()];
    }

    scores.into_iter().map(|score| score / sum).collect()
}

fn l1_delta(previous: &[f32], current: &[f32]) -> f32 {
    previous
        .iter()
        .zip(current.iter())
        .map(|(old, new)| (old - new).abs())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{rank_sentences, LexRankError, LexRankOptions};
    use crate::pipeline::sentence::SentenceNode;
    use std::collections::HashMap;

    fn node(position: usize, text: &str, tokens: &[&str]) -> SentenceNode {
        let mut bag = HashMap::new();
        for token in tokens {
            *bag.entry((*token).to_string()).or_insert(0.0) += 1.0;
        }

        SentenceNode::new(
            position,
            text.to_string(),
            tokens.iter().map(|token| (*token).to_string()).collect(),
            bag,
        )
    }

    fn default_options() -> LexRankOptions {
        LexRankOptions {
            similarity_threshold: 0.1,
            damping_factor: 0.85,
            max_iterations: 100,
            convergence_delta: 1e-4,
        }
    }

    #[test]
    fn ranks_sentences_by_similarity() {
        let sentences = vec![
            node(
                0,
                "Rust focuses on safety and concurrency.",
                &["rust", "safety", "concurrency"],
            ),
            node(
                1,
                "Rust delivers great performance for systems.",
                &["rust", "performance", "systems"],
            ),
            node(
                2,
                "Cooking recipes require fresh ingredients.",
                &["cooking", "recipe", "ingredients"],
            ),
        ];

        let ranked = rank_sentences(&sentences, &default_options()).expect("lexrank");

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].position, 0);
        assert!(ranked[0].score >= ranked[1].score);
        assert!(ranked[1].score > ranked[2].score);
    }

    #[test]
    fn returns_empty_for_no_sentences() {
        let ranked = rank_sentences(&[], &default_options()).expect("lexrank");
        assert!(ranked.is_empty());
    }

    #[test]
    fn errors_when_not_converged() {
        let sentences = vec![node(0, "A", &["a"]), node(1, "B", &["b"])];

        let options = LexRankOptions {
            similarity_threshold: 0.9,
            damping_factor: 0.85,
            max_iterations: 0,
            convergence_delta: 1e-12,
        };

        let result = rank_sentences(&sentences, &options);

        match result {
            Err(LexRankError::DidNotConverge { iterations, .. }) => {
                assert_eq!(iterations, 0);
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }
}
