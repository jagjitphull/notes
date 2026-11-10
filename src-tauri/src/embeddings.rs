//! Local semantic search via Ollama's embeddings API
//! (https://127.0.0.1:11434, no auth, nothing leaves the machine). Every
//! call here is best-effort: Ollama not being installed or running is the
//! expected common case, not an error condition, so callers treat any
//! failure the same way — Smart Search stays hidden or falls back to FTS5.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

/// The user needs to `ollama pull nomic-embed-text` once. If the model is
/// missing, Ollama's own error surfaces through `embed` and is treated the
/// same as "unavailable" by callers.
pub const MODEL: &str = "nomic-embed-text";

const BASE_URL: &str = "http://127.0.0.1:11434";

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(300))
        .timeout(timeout)
        .build()
}

/// Fast reachability check so the UI can decide whether to show the Smart
/// Search toggle at all, without ever stalling on a missing/stopped Ollama.
pub fn is_available() -> bool {
    agent(Duration::from_millis(500))
        .get(&format!("{BASE_URL}/api/tags"))
        .call()
        .is_ok()
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

/// Embeds one piece of text. Local inference, so a note-sized chunk of
/// text is fast, but a large vault's first indexing pass still means many
/// sequential calls — hence the generous request timeout.
pub fn embed(text: &str) -> Result<Vec<f32>, String> {
    let response = agent(Duration::from_secs(30))
        .post(&format!("{BASE_URL}/api/embeddings"))
        .send_json(json!({ "model": MODEL, "prompt": text }))
        .map_err(|e| e.to_string())?;
    let parsed: EmbedResponse = response.into_json().map_err(|e| e.to_string())?;
    Ok(parsed.embedding)
}

pub fn vector_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

// `slice::as_chunks` (clippy's suggested replacement for `chunks_exact`
// here) postdates this crate's declared MSRV (1.90), so `chunks_exact`
// stays - it's not a correctness concern, just a lint from a newer
// clippy than what this crate targets. `unknown_lints` is allowed too
// since older clippy (this crate's MSRV) doesn't recognize the lint name
// below at all.
#[allow(unknown_lints)]
#[allow(clippy::chunks_exact_to_as_chunks)]
pub fn bytes_to_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Ranks embeddings by similarity to `query`, brute-force (no ANN index —
/// a personal notes vault is small enough that a linear scan over a few
/// thousand short vectors is effectively instant).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_bytes_round_trip() {
        let v = vec![0.5_f32, -1.25, 3.0, 0.0];
        let bytes = vector_to_bytes(&v);
        assert_eq!(bytes.len(), 16);
        assert_eq!(bytes_to_vector(&bytes), v);
    }

    #[test]
    fn cosine_similarity_of_identical_vectors_is_one() {
        let v = vec![1.0_f32, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_of_orthogonal_vectors_is_zero() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_handles_zero_vector() {
        let a = vec![0.0_f32, 0.0];
        let b = vec![1.0_f32, 2.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }
}
