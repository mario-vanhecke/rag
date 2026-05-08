//! `rag prune` removes registry rows in non-`indexed` states.

mod common;

use common::{run_index_stub, write};
use rag_core::index::IndexOptions;
use rag_core::registry::{add_paths, prune, AddOptions, FileStatus, PruneOptions};
use rag_core::Vault;

fn count(v: &Vault, sql: &str) -> i64 {
    v.conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn setup_with_mixed_statuses() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = Vault::init(dir.path(), false).unwrap();
    write(dir.path(), "ok.md", "# OK\n\nbody.");
    write(dir.path(), "empty.md", ""); // → failed
    write(dir.path(), "delete_me.md", "# X\n\nbody.");
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
    // Now: ok.md=indexed, empty.md=failed, delete_me.md=indexed.
    std::fs::remove_file(dir.path().join("delete_me.md")).unwrap();
    run_index_stub(&mut vault, IndexOptions::default());
    // Now: ok.md=indexed, empty.md=failed, delete_me.md=missing.
    (dir, vault)
}

#[test]
fn default_targets_missing() {
    let (_dir, vault) = setup_with_mixed_statuses();
    let r = prune(&vault, &PruneOptions::default()).unwrap();
    assert_eq!(r.removed, 1);
    assert_eq!(count(&vault, "SELECT COUNT(*) FROM files"), 2);
    assert_eq!(
        count(&vault, "SELECT COUNT(*) FROM files WHERE status='missing'"),
        0
    );
}

#[test]
fn dry_run_makes_no_changes() {
    let (_dir, vault) = setup_with_mixed_statuses();
    let before = count(&vault, "SELECT COUNT(*) FROM files");
    let r = prune(
        &vault,
        &PruneOptions {
            dry_run: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.removed, 1, "would remove 1 missing row");
    assert_eq!(count(&vault, "SELECT COUNT(*) FROM files"), before);
}

#[test]
fn status_filter_targets_specific_state() {
    let (_dir, vault) = setup_with_mixed_statuses();
    let r = prune(
        &vault,
        &PruneOptions {
            status: Some(FileStatus::Failed),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.removed, 1, "the failed row");
    assert_eq!(
        count(&vault, "SELECT COUNT(*) FROM files WHERE status='failed'"),
        0
    );
    assert_eq!(count(&vault, "SELECT COUNT(*) FROM files"), 2);
}

#[test]
fn all_non_indexed_clears_everything_except_indexed() {
    let (_dir, vault) = setup_with_mixed_statuses();
    let r = prune(
        &vault,
        &PruneOptions {
            all_non_indexed: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.removed, 2, "missing + failed");
    assert_eq!(
        count(&vault, "SELECT COUNT(*) FROM files"),
        1,
        "only the indexed row remains"
    );
}

#[test]
fn prune_does_not_remove_indexed_rows() {
    let (_dir, vault) = setup_with_mixed_statuses();
    let _ = prune(
        &vault,
        &PruneOptions {
            all_non_indexed: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        count(&vault, "SELECT COUNT(*) FROM files WHERE status='indexed'"),
        1,
        "indexed rows are never pruned"
    );
}
