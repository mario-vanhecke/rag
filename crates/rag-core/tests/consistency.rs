//! Consistency invariant: chunks exist if and only if the corresponding files
//! row has status='indexed'.

mod common;

use common::{run_index_stub, write, StubEmbedder};
use rag_core::config::Config;
use rag_core::embed::Embedder;
use rag_core::extract::ExtractorRegistry;
use rag_core::index::{run_index, IndexOptions};
use rag_core::registry::{add_paths, AddOptions};
use rag_core::Vault;
use std::path::PathBuf;

fn open() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::init(dir.path(), false).unwrap();
    (dir, vault)
}

fn count(v: &Vault, sql: &str) -> i64 {
    v.conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn invariants_hold(v: &Vault) {
    let chunks = count(v, "SELECT COUNT(*) FROM chunks");
    let vectors = count(v, "SELECT COUNT(*) FROM chunk_vectors");
    let fts = count(v, "SELECT COUNT(*) FROM chunk_fts");
    assert_eq!(chunks, vectors, "vectors must match chunks");
    assert_eq!(chunks, fts, "fts must match chunks");
    let orphans = count(
        v,
        "SELECT COUNT(*) FROM chunks c
         LEFT JOIN files f ON f.id = c.file_id
         WHERE f.id IS NULL OR f.status != 'indexed'",
    );
    assert_eq!(orphans, 0, "no chunks for non-indexed files");
}

#[test]
fn invariant_after_normal_index() {
    let (dir, mut vault) = open();
    write(dir.path(), "a.md", "# A\n\nbody one two three.");
    write(dir.path(), "b.md", "# B\n\nbody four five six.");

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
    invariants_hold(&vault);
    assert_eq!(
        count(&vault, "SELECT COUNT(*) FROM files WHERE status='indexed'"),
        2
    );
}

#[test]
fn invariant_after_file_deletion() {
    let (dir, mut vault) = open();
    write(dir.path(), "a.md", "# A\n\nbody one.");
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
    invariants_hold(&vault);
    assert!(count(&vault, "SELECT COUNT(*) FROM chunks") > 0);

    // Delete the file from disk and re-index → status should become 'missing'
    // and ALL its chunks should be gone in the same transaction.
    std::fs::remove_file(dir.path().join("a.md")).unwrap();
    run_index_stub(&mut vault, IndexOptions::default());

    invariants_hold(&vault);
    assert_eq!(count(&vault, "SELECT COUNT(*) FROM chunks"), 0);
    assert_eq!(
        count(&vault, "SELECT COUNT(*) FROM files WHERE status='missing'"),
        1
    );
}

#[test]
fn invariant_after_embedder_failure() {
    // An embedder that always errors. Per the spec, a failed embedding must
    // leave the file in 'failed' state with NO chunks written.
    struct FailingEmbedder;
    impl Embedder for FailingEmbedder {
        fn dimension(&self) -> u32 {
            1024
        }
        fn model_id(&self) -> &str {
            "failing"
        }
        fn embed_batch(&self, _: &[&str]) -> rag_core::Result<Vec<Vec<f32>>> {
            Err(rag_core::Error::embedder("simulated embedding failure"))
        }
    }

    let (dir, mut vault) = open();
    write(dir.path(), "a.md", "# A\n\nbody.");
    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();

    let extractors = ExtractorRegistry::standard();
    let _ = run_index(
        &mut vault,
        &FailingEmbedder,
        &extractors,
        &IndexOptions::default(),
        None,
    )
    .unwrap();

    invariants_hold(&vault);
    assert_eq!(count(&vault, "SELECT COUNT(*) FROM chunks"), 0);
    assert_eq!(
        count(&vault, "SELECT COUNT(*) FROM files WHERE status='failed'"),
        1
    );
}

#[test]
fn invariant_after_status_change_to_excluded() {
    let (dir, mut vault) = open();
    write(dir.path(), "a.txt", "plain content body.");

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
    assert!(count(&vault, "SELECT COUNT(*) FROM chunks") > 0);

    // Now exclude .txt via config. Re-index transitions out of 'indexed' →
    // 'excluded' and must drop chunks.
    Config::set(
        &vault.conn,
        "files.excluded_extensions",
        serde_json::json!(["txt"]),
    )
    .unwrap();
    let mut vault = Vault::open(dir.path()).unwrap();
    run_index_stub(&mut vault, IndexOptions::default());

    invariants_hold(&vault);
    assert_eq!(count(&vault, "SELECT COUNT(*) FROM chunks"), 0);
    assert_eq!(
        count(&vault, "SELECT COUNT(*) FROM files WHERE status='excluded'"),
        1
    );
}

#[test]
fn invariant_chunks_replaced_atomically_on_reindex() {
    let (dir, mut vault) = open();
    write(dir.path(), "a.md", "# A\n\nfirst version.");
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

    let initial = count(&vault, "SELECT COUNT(*) FROM chunks");
    let initial_ids: Vec<String> = vault
        .conn
        .prepare("SELECT id FROM chunks ORDER BY ordinal")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    // Edit the file with substantial new content + bump mtime.
    write(
        dir.path(),
        "a.md",
        "# A\n\n## sub\n\nbrand new content one.\n\n## sub2\n\nmore content two.",
    );
    common::touch_future(&dir.path().join("a.md"));

    run_index_stub(&mut vault, IndexOptions::default());
    invariants_hold(&vault);

    let after_ids: Vec<String> = vault
        .conn
        .prepare("SELECT id FROM chunks ORDER BY ordinal")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let intersection = initial_ids
        .iter()
        .filter(|id| after_ids.contains(id))
        .count();
    assert_eq!(
        intersection, 0,
        "old chunk IDs must all be gone after reindex"
    );
    let _ = initial;
    let _ = std::any::type_name::<StubEmbedder>(); // touch the import
    let _ = PathBuf::new();
}
