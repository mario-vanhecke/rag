//! Files with legitimate duplicate content (chapter separators, empty
//! section headers, etc.) must index without tripping a uniqueness
//! constraint. Regression test for v0.1.3.

mod common;

use common::{run_index_stub, write};
use rag_core::index::{IndexOptions, Outcome};
use rag_core::registry::{add_paths, AddOptions};
use rag_core::Vault;

#[test]
fn file_with_duplicate_section_content_indexes_successfully() {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = Vault::init(dir.path(), false).unwrap();

    // A book-like markdown with multiple chapters, each ending with the
    // same separator content. Pre-v0.1.3 this hit the
    // (file_id, content_hash) UNIQUE index and aborted the whole run.
    let body = "# Book\n\n\
        ## Chapter 1\n\nFirst chapter content.\n\n* * *\n\n\
        ## Chapter 2\n\nSecond chapter content.\n\n* * *\n\n\
        ## Chapter 3\n\nThird chapter content.\n\n* * *\n";
    write(dir.path(), "book.md", body);
    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();

    let report = run_index_stub(&mut vault, IndexOptions::default());
    assert_eq!(
        report.summary.failed, 0,
        "duplicate content within a file must not fail the run: {:?}",
        report.results
    );
    assert_eq!(report.summary.indexed, 1);

    let chunks: i64 = vault
        .conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
        .unwrap();
    assert!(chunks > 1, "expected several chunks; got {}", chunks);
}

#[test]
fn one_bad_file_does_not_abort_the_whole_run() {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = Vault::init(dir.path(), false).unwrap();

    // Three good files, plus one with duplicate-content sections (which on
    // older schemas would crash the run). With v0.1.3 the duplicate-content
    // file indexes fine; in any case, sibling files must finish.
    write(dir.path(), "a.md", "# A\n\nbody a.");
    write(dir.path(), "b.md", "# B\n\nbody b.");
    write(
        dir.path(),
        "c.md",
        "# C\n## s1\n\n* * *\n\n## s2\n\n* * *\n\n## s3\n\n* * *",
    );
    write(dir.path(), "d.md", "# D\n\nbody d.");
    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();

    let report = run_index_stub(&mut vault, IndexOptions::default());
    let indexed: Vec<&str> = report
        .results
        .iter()
        .filter(|r| r.outcome == Outcome::Indexed)
        .map(|r| r.path.as_str())
        .collect();
    assert!(indexed.contains(&"a.md"));
    assert!(indexed.contains(&"b.md"));
    assert!(indexed.contains(&"d.md"));
}
