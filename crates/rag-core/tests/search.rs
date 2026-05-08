//! Hybrid search: vector + keyword + RRF fusion.

mod common;

use common::{run_index_stub, write, StubEmbedder};
use rag_core::index::IndexOptions;
use rag_core::registry::{add_paths, AddOptions};
use rag_core::search::{search, SearchMode, SearchQuery};
use rag_core::Vault;

fn setup() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = Vault::init(dir.path(), false).unwrap();
    write(
        dir.path(),
        "branching.md",
        "# Branching\n\n## Trunk-based\n\nWe use trunk-based development with short-lived feature branches.",
    );
    write(
        dir.path(),
        "release.md",
        "# Release process\n\n## Cadence\n\nWe ship a release every two weeks.",
    );
    write(
        dir.path(),
        "notes.md",
        "# Engineering notes\n\n## Q3\n\nIntegration tests over mocks.",
    );
    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();
    run_index_stub(&mut vault, IndexOptions::default());
    (dir, vault)
}

fn q(text: &str, k: u32, mode: SearchMode) -> SearchQuery {
    SearchQuery {
        query: text.to_string(),
        k,
        filter: None,
        mode,
        threshold: None,
    }
}

#[test]
fn keyword_only_finds_exact_term() {
    let (_dir, vault) = setup();
    let hits = search(
        &vault,
        &StubEmbedder::new(),
        &q("trunk", 5, SearchMode::KeywordOnly),
    )
    .unwrap();
    assert!(!hits.is_empty(), "should find 'trunk'");
    assert_eq!(hits[0].file_path, "branching.md");
}

#[test]
fn vector_only_returns_results() {
    let (_dir, vault) = setup();
    let hits = search(
        &vault,
        &StubEmbedder::new(),
        &q("anything", 5, SearchMode::VectorOnly),
    )
    .unwrap();
    assert!(
        !hits.is_empty(),
        "vector search returns top-k regardless of relevance"
    );
}

#[test]
fn glob_filter_restricts_to_pattern() {
    let (_dir, vault) = setup();
    let mut q = q("release", 10, SearchMode::Hybrid);
    q.filter = Some("release.md".to_string());
    let hits = search(&vault, &StubEmbedder::new(), &q).unwrap();
    for h in &hits {
        assert_eq!(h.file_path, "release.md", "filter should restrict");
    }
}

#[test]
fn glob_filter_with_no_matches_returns_empty() {
    let (_dir, vault) = setup();
    let mut q = q("release", 10, SearchMode::Hybrid);
    q.filter = Some("nonexistent*.md".to_string());
    let hits = search(&vault, &StubEmbedder::new(), &q).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn threshold_filters_low_scores() {
    let (_dir, vault) = setup();
    let mut q = q("release", 10, SearchMode::Hybrid);
    q.threshold = Some(99.0);
    let hits = search(&vault, &StubEmbedder::new(), &q).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn k_caps_result_count() {
    let (_dir, vault) = setup();
    let hits = search(
        &vault,
        &StubEmbedder::new(),
        &q("the", 1, SearchMode::Hybrid),
    )
    .unwrap();
    assert!(hits.len() <= 1);
}

#[test]
fn fts_metacharacters_in_query_dont_panic() {
    let (_dir, vault) = setup();
    // FTS5 has its own query language; a raw user query with operators must
    // not panic the search.
    let hits = search(
        &vault,
        &StubEmbedder::new(),
        &q("OR AND \"weird\" *", 5, SearchMode::KeywordOnly),
    )
    .unwrap();
    let _ = hits; // we just care that it returns
}

#[test]
fn empty_query_does_not_panic() {
    let (_dir, vault) = setup();
    let hits = search(
        &vault,
        &StubEmbedder::new(),
        &q("", 5, SearchMode::KeywordOnly),
    )
    .unwrap();
    assert!(hits.is_empty(), "empty FTS query yields no hits");
}
