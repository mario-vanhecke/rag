//! End-to-end test of the non-embedder lifecycle: init → add → status → rm → prune.
//! Indexing requires a model download which we skip in CI.

use rag_core::registry::{add_paths, prune, remove_paths, AddOptions, FileStatus, PruneOptions};
use rag_core::status::{compute, StatusOptions};
use rag_core::Vault;

fn write(path: &std::path::Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

#[test]
fn lifecycle_without_indexing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // init
    let vault = Vault::init(root, false).unwrap();
    let (vid, _, version) = vault.meta().unwrap();
    assert!(!vid.is_empty());
    assert_eq!(version, env!("CARGO_PKG_VERSION"));

    // add
    write(&root.join("docs/a.md"), "# A\n\nbody");
    write(&root.join("docs/b.md"), "# B\n\nbody");
    write(&root.join("docs/cover.png"), "fake png");

    let report = add_paths(
        &vault,
        &[root.join("docs")],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(report.added, 2);
    assert_eq!(report.skipped_unsupported, 1);

    // status
    let s = compute(
        &vault,
        &StatusOptions {
            no_stat: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s.summary.registered, 2);
    assert_eq!(s.summary.pending, 2);

    // rm
    let rm = remove_paths(&vault, &["docs/a.md".to_string()]).unwrap();
    assert_eq!(rm.removed, 1);

    // prune (default targets `missing` — there are none, so 0)
    let p = prune(
        &vault,
        &PruneOptions {
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(p.removed, 0);

    // prune all non-indexed: removes the remaining pending row
    let p = prune(
        &vault,
        &PruneOptions {
            all_non_indexed: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(p.removed, 1);

    // After prune, registry is empty.
    let s = compute(
        &vault,
        &StatusOptions {
            no_stat: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s.summary.registered, 0);
}

#[test]
fn add_excluded_extensions() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let vault = Vault::init(root, false).unwrap();

    // Configure 'txt' as excluded
    rag_core::config::Config::set(
        &vault.conn,
        "files.excluded_extensions",
        serde_json::json!(["txt"]),
    )
    .unwrap();

    write(&root.join("a.md"), "x");
    write(&root.join("b.txt"), "y");

    // Reload vault to pick up config
    drop(vault);
    let vault = Vault::open(root).unwrap();
    let r = add_paths(&vault, &[root.to_path_buf()], &AddOptions::default()).unwrap();
    assert_eq!(r.added, 2, "both files registered");

    // After indexing they would split into indexed/excluded; without indexing,
    // they're both pending.
    let s = compute(
        &vault,
        &StatusOptions {
            no_stat: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s.summary.pending, 2);
    let _ = FileStatus::Excluded; // silence unused if test doesn't reach it
}
