use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Pending,
    Indexed,
    Failed,
    Unsupported,
    Excluded,
    TooLarge,
    NeedsOcr,
    Missing,
}

impl FileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Indexed => "indexed",
            Self::Failed => "failed",
            Self::Unsupported => "unsupported",
            Self::Excluded => "excluded",
            Self::TooLarge => "too_large",
            Self::NeedsOcr => "needs_ocr",
            Self::Missing => "missing",
        }
    }
    pub fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "pending" => Self::Pending,
            "indexed" => Self::Indexed,
            "failed" => Self::Failed,
            "unsupported" => Self::Unsupported,
            "excluded" => Self::Excluded,
            "too_large" => Self::TooLarge,
            "needs_ocr" => Self::NeedsOcr,
            "missing" => Self::Missing,
            other => return Err(Error::other(format!("unknown status: {other}"))),
        })
    }
    pub fn has_chunks(&self) -> bool {
        matches!(self, Self::Indexed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        for s in [
            FileStatus::Pending,
            FileStatus::Indexed,
            FileStatus::Failed,
            FileStatus::Unsupported,
            FileStatus::Excluded,
            FileStatus::TooLarge,
            FileStatus::NeedsOcr,
            FileStatus::Missing,
        ] {
            assert_eq!(FileStatus::from_str(s.as_str()).unwrap(), s);
        }
    }
    #[test]
    fn only_indexed_has_chunks() {
        assert!(FileStatus::Indexed.has_chunks());
        for s in [
            FileStatus::Pending,
            FileStatus::Failed,
            FileStatus::Unsupported,
            FileStatus::Excluded,
            FileStatus::TooLarge,
            FileStatus::NeedsOcr,
            FileStatus::Missing,
        ] {
            assert!(!s.has_chunks());
        }
    }
}
