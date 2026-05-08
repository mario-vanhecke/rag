//! Each non-`indexed` status is reachable; resolving the condition recovers.

mod common;

use common::{run_index_stub, write};
use rag_core::config::Config;
use rag_core::index::IndexOptions;
use rag_core::registry::{add_paths, AddOptions, FileStatus};
use rag_core::Vault;

fn open() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::init(dir.path(), false).unwrap();
    (dir, vault)
}

fn count(v: &Vault, status: FileStatus) -> i64 {
    v.conn
        .query_row(
            "SELECT COUNT(*) FROM files WHERE status = ?1",
            rag_core::rusqlite::params![status.as_str()],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn pending_to_indexed() {
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
    assert_eq!(count(&vault, FileStatus::Pending), 1);
    run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(count(&vault, FileStatus::Indexed), 1);
}

#[test]
fn empty_file_to_failed_then_recovers_when_filled() {
    let (dir, mut vault) = open();
    write(dir.path(), "a.md", "");
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
    assert_eq!(count(&vault, FileStatus::Failed), 1);

    write(dir.path(), "a.md", "# A\n\nbody now.");
    common::touch_future(&dir.path().join("a.md"));
    run_index_stub(
        &mut vault,
        IndexOptions {
            retry_failed: true,
            ..Default::default()
        },
    );
    assert_eq!(count(&vault, FileStatus::Indexed), 1);
    assert_eq!(count(&vault, FileStatus::Failed), 0);
}

#[test]
fn unsupported_extension_then_supported_after_config() {
    let (dir, mut vault) = open();
    // .json is not in default supported extensions.
    write(dir.path(), "a.json", "{\"x\":1}");
    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        // skip_unsupported=false so the row IS created.
        &AddOptions::default(),
    )
    .unwrap();
    run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(count(&vault, FileStatus::Unsupported), 1);

    // Add .json to supported. Even though .json has no registered extractor,
    // re-index should now error for a different reason ('failed') — meaning
    // the unsupported transition is reversible.
    Config::set(
        &vault.conn,
        "files.supported_extensions",
        serde_json::json!(["md", "markdown", "docx", "pdf", "txt", "json"]),
    )
    .unwrap();
    let mut vault = Vault::open(dir.path()).unwrap();
    run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(count(&vault, FileStatus::Unsupported), 0);
}

#[test]
fn excluded_via_config_then_indexed_after_unset() {
    let (dir, vault) = open();
    write(dir.path(), "a.txt", "plaintext content body.");
    add_paths(&vault, &[dir.path().to_path_buf()], &AddOptions::default()).unwrap();

    Config::set(
        &vault.conn,
        "files.excluded_extensions",
        serde_json::json!(["txt"]),
    )
    .unwrap();
    let mut vault = Vault::open(dir.path()).unwrap();
    run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(count(&vault, FileStatus::Excluded), 1);

    Config::set(
        &vault.conn,
        "files.excluded_extensions",
        serde_json::json!([]),
    )
    .unwrap();
    let mut vault = Vault::open(dir.path()).unwrap();
    run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(count(&vault, FileStatus::Indexed), 1);
}

#[test]
fn too_large_then_size_cap_raised() {
    let (dir, vault) = open();
    let _ = vault;
    // Write a 100KB file. Cap at 1KB → too_large.
    let body = "x".repeat(100_000);
    write(dir.path(), "big.md", &format!("# Big\n\n{body}"));
    add_paths(&vault, &[dir.path().to_path_buf()], &AddOptions::default()).unwrap();

    Config::set(&vault.conn, "files.size_cap_bytes", serde_json::json!(1024)).unwrap();
    let mut vault = Vault::open(dir.path()).unwrap();
    run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(count(&vault, FileStatus::TooLarge), 1);

    // Raise cap. Re-index → indexed.
    Config::set(
        &vault.conn,
        "files.size_cap_bytes",
        serde_json::json!(10_000_000),
    )
    .unwrap();
    let mut vault = Vault::open(dir.path()).unwrap();
    run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(count(&vault, FileStatus::Indexed), 1);
}

#[test]
fn missing_then_restored() {
    let (dir, mut vault) = open();
    write(dir.path(), "a.md", "# A\n\nbody one two three.");
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
    assert_eq!(count(&vault, FileStatus::Indexed), 1);

    std::fs::remove_file(dir.path().join("a.md")).unwrap();
    run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(count(&vault, FileStatus::Missing), 1);

    write(dir.path(), "a.md", "# A back\n\nfresh content.");
    run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(count(&vault, FileStatus::Indexed), 1);
    assert_eq!(count(&vault, FileStatus::Missing), 0);
}
