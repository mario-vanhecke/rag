pub mod candle;

use crate::error::Result;

pub trait Embedder: Send + Sync {
    fn dimension(&self) -> u32;
    fn model_id(&self) -> &str;
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}

pub use candle::CandleEmbedder;
