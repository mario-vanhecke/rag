//! Shared test helpers. Importantly: a deterministic stub embedder so tests
//! don't need the 2.2 GB bge-m3 download.
//!
//! Each integration test that uses these helpers must declare:
//! ```
//! mod common;
//! ```

#![allow(dead_code)]

use rag_core::embed::Embedder;
use rag_core::extract::ExtractorRegistry;
use rag_core::index::{run_index, IndexOptions, IndexReport};
use rag_core::Vault;
use std::path::Path;
use std::sync::Mutex;

/// 1024-dim deterministic embedder. Produces unit-norm vectors derived from the
/// SHA-256 of the input text, so identical text → identical vector and similar
/// text often shares early bytes (rough cosine signal).
pub struct StubEmbedder {
    pub calls: Mutex<u32>,
}

impl StubEmbedder {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(0),
        }
    }
}

impl Default for StubEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl Embedder for StubEmbedder {
    fn dimension(&self) -> u32 {
        1024
    }
    fn model_id(&self) -> &str {
        "stub"
    }
    fn embed_batch(&self, texts: &[&str]) -> rag_core::Result<Vec<Vec<f32>>> {
        use sha2::{Digest, Sha256};
        *self.calls.lock().unwrap() += 1;
        let mut out = Vec::with_capacity(texts.len());
        for t in texts {
            let mut v = vec![0.0_f32; 1024];
            // Seed each dim from a chained hash of the text. Cheap, deterministic.
            let mut seed = Sha256::digest(t.as_bytes());
            for chunk in v.chunks_mut(32) {
                for (i, byte) in seed.iter().take(chunk.len()).enumerate() {
                    chunk[i] = (*byte as f32 / 255.0) * 2.0 - 1.0;
                }
                seed = Sha256::digest(seed);
            }
            // L2-normalize so vec_distance_cosine behaves well.
            let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
            for x in &mut v {
                *x /= n;
            }
            out.push(v);
        }
        Ok(out)
    }
}

/// Run `rag index` with the stub embedder and standard extractors.
pub fn run_index_stub(vault: &mut Vault, opts: IndexOptions) -> IndexReport {
    let embedder = StubEmbedder::new();
    let extractors = ExtractorRegistry::standard();
    run_index(vault, &embedder, &extractors, &opts, None).expect("index should not error")
}

/// Write a file under the vault root, creating parent dirs.
pub fn write(root: &Path, rel: &str, body: &str) {
    let p = root.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(p, body).unwrap();
}

/// Set the file's mtime to a known instant, so tests can simulate "edited".
pub fn touch_future(path: &Path) {
    let when = std::time::SystemTime::now() + std::time::Duration::from_secs(1000);
    let _ = filetime::set_file_mtime(path, filetime::FileTime::from_system_time(when));
}
