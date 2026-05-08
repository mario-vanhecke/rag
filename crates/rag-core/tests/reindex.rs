//! Re-indexing semantics: only modified files get reprocessed; unmodified
//! files are skipped.

mod common;

use common::{run_index_stub, touch_future, write};
use rag_core::index::{IndexOptions, Outcome};
use rag_core::registry::{add_paths, AddOptions};
use rag_core::Vault;

fn setup() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::init(dir.path(), false).unwrap();
    write(dir.path(), "a.md", "# A\n\noriginal a content.");
    write(dir.path(), "b.md", "# B\n\noriginal b content.");
    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();
    (dir, vault)
}

#[test]
fn second_run_is_a_no_op() {
    let (_dir, mut vault) = setup();
    let r1 = run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(r1.summary.indexed, 2);

    let r2 = run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(r2.summary.indexed, 0, "no files re-indexed second time");
    assert_eq!(r2.summary.skipped, 2, "all skipped");
}

#[test]
fn modified_file_is_reprocessed() {
    let (dir, mut vault) = setup();
    run_index_stub(&mut vault, IndexOptions::default());

    // Edit b.md; bump mtime explicitly so the test is deterministic on macOS
    // where the system clock might land on the same second as the initial write.
    write(dir.path(), "b.md", "# B\n\nedited b content.");
    touch_future(&dir.path().join("b.md"));

    let r = run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(r.summary.indexed, 1, "exactly the edited file");
    assert_eq!(r.summary.skipped, 1, "the unchanged file was skipped");

    // The Indexed outcome must be for b.md.
    let indexed_paths: Vec<&str> = r
        .results
        .iter()
        .filter(|f| f.outcome == Outcome::Indexed)
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(indexed_paths, vec!["b.md"]);
}

#[test]
fn force_reindexes_everything() {
    let (_dir, mut vault) = setup();
    run_index_stub(&mut vault, IndexOptions::default());

    let r = run_index_stub(
        &mut vault,
        IndexOptions {
            force: true,
            ..Default::default()
        },
    );
    assert_eq!(r.summary.indexed, 2);
    assert_eq!(r.summary.skipped, 0);
}

#[test]
fn paths_filter_restricts_processing() {
    let (dir, mut vault) = setup();
    run_index_stub(&mut vault, IndexOptions::default());
    write(dir.path(), "a.md", "# A\n\nedited a content.");
    write(dir.path(), "b.md", "# B\n\nedited b content.");
    touch_future(&dir.path().join("a.md"));
    touch_future(&dir.path().join("b.md"));

    let r = run_index_stub(
        &mut vault,
        IndexOptions {
            paths: Some(vec![dir.path().join("a.md")]),
            ..Default::default()
        },
    );
    assert_eq!(r.summary.indexed, 1);
    assert_eq!(
        r.results
            .iter()
            .find(|f| f.outcome == Outcome::Indexed)
            .unwrap()
            .path,
        "a.md"
    );
}

#[test]
fn failed_files_are_skipped_unless_retry_failed() {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = Vault::init(dir.path(), false).unwrap();
    // Empty markdown will produce no chunks → 'failed'.
    write(dir.path(), "empty.md", "");
    write(dir.path(), "ok.md", "# OK\n\nbody body body.");
    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();

    let r = run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(r.summary.failed, 1);
    assert_eq!(r.summary.indexed, 1);

    // Re-run: the failed file is skipped, not retried.
    let r = run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(r.summary.failed, 0);
    assert_eq!(
        r.summary.skipped, 2,
        "both ok.md (indexed) and empty.md (failed) skipped"
    );

    // With --retry-failed: empty.md is retried, fails again.
    let r = run_index_stub(
        &mut vault,
        IndexOptions {
            retry_failed: true,
            ..Default::default()
        },
    );
    assert_eq!(r.summary.failed, 1);
}
