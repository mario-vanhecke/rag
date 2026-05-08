//! Two `rag index` invocations must not corrupt the database. With
//! `--no-wait`, the loser exits with LockContention.

mod common;

use common::{write, StubEmbedder};
use rag_core::extract::ExtractorRegistry;
use rag_core::index::{run_index, IndexOptions};
use rag_core::registry::{add_paths, AddOptions};
use rag_core::Vault;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn no_wait_loser_returns_lock_contention() {
    use fs2::FileExt;
    use std::fs::OpenOptions;

    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::init(dir.path(), false).unwrap();
    write(dir.path(), "a.md", "# A\n\nbody body body.");
    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();

    // Hold the index lock exclusively so any concurrent rag-index call must
    // observe contention. The stub embedder makes the legitimate index path
    // too fast for thread-vs-thread racing to be reliable; this is the
    // deterministic equivalent.
    let lock_path = vault.index_lock_path();
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .unwrap();
    lock_file.lock_exclusive().unwrap();

    // The vault holds an open Connection that we don't want to share with the
    // other thread, so drop it and reopen inside the spawned thread.
    drop(vault);

    let p = Arc::new(dir.path().to_path_buf());
    let p2 = p.clone();
    let h = std::thread::spawn(move || {
        let mut vault = Vault::open(&p2).unwrap();
        run_index(
            &mut vault,
            &StubEmbedder::new(),
            &ExtractorRegistry::standard(),
            &IndexOptions {
                no_wait: true,
                ..Default::default()
            },
            None,
        )
    });

    let res = h.join().unwrap();
    match res {
        Err(rag_core::Error::LockContention) => {} // expected
        other => panic!("expected LockContention, got {other:?}"),
    }

    // Release for cleanup.
    let _ = FileExt::unlock(&lock_file);
    let _ = p; // silence warning
}

#[test]
fn waiter_eventually_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    {
        let vault = Vault::init(dir.path(), false).unwrap();
        write(dir.path(), "a.md", "# A\n\nbody body body.");
        add_paths(
            &vault,
            &[dir.path().to_path_buf()],
            &AddOptions {
                skip_unsupported: true,
                ..Default::default()
            },
        )
        .unwrap();
    }

    let dir_path = Arc::new(dir.path().to_path_buf());
    let p1 = dir_path.clone();
    let p2 = dir_path.clone();

    let h1 = std::thread::spawn(move || {
        let mut vault = Vault::open(&p1).unwrap();
        run_index(
            &mut vault,
            &StubEmbedder::new(),
            &ExtractorRegistry::standard(),
            &IndexOptions::default(),
            None,
        )
    });

    std::thread::sleep(Duration::from_millis(20));

    let h2 = std::thread::spawn(move || {
        let mut vault = Vault::open(&p2).unwrap();
        run_index(
            &mut vault,
            &StubEmbedder::new(),
            &ExtractorRegistry::standard(),
            &IndexOptions {
                wait_seconds: Some(10),
                ..Default::default()
            },
            None,
        )
    });

    assert!(h1.join().unwrap().is_ok());
    assert!(
        h2.join().unwrap().is_ok(),
        "waiter should succeed once first releases lock"
    );
}
