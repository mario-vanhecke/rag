//! .vaultignore + built-in defaults behavior.

mod common;

use common::write;
use rag_core::registry::{add_paths, AddOptions};
use rag_core::Vault;

fn setup() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::init(dir.path(), false).unwrap();
    (dir, vault)
}

fn registered_paths(vault: &Vault) -> Vec<String> {
    let mut stmt = vault
        .conn
        .prepare("SELECT path FROM files ORDER BY path")
        .unwrap();
    let v: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    v
}

#[test]
fn builtin_defaults_exclude_git_node_modules_etc() {
    let (dir, vault) = setup();
    write(dir.path(), "docs/r1.md", "# r1");
    write(dir.path(), "docs/r2.md", "# r2");
    write(dir.path(), "node_modules/p.json", "x");
    write(dir.path(), ".git/config", "x");
    write(dir.path(), "__pycache__/x.pyc", "x");
    write(dir.path(), ".DS_Store", "x");
    write(dir.path(), "foo.pyc", "x");

    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        registered_paths(&vault),
        vec!["docs/r1.md".to_string(), "docs/r2.md".to_string()]
    );
}

#[test]
fn vaultignore_directory_pattern() {
    let (dir, vault) = setup();
    write(dir.path(), "docs/r.md", "# r");
    write(dir.path(), "drafts/d.md", "# d");
    write(dir.path(), "archive/a.md", "# a");
    write(dir.path(), ".vaultignore", "drafts/\narchive/\n");

    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(registered_paths(&vault), vec!["docs/r.md".to_string()]);
}

#[test]
fn vaultignore_glob_with_negation() {
    let (dir, vault) = setup();
    write(dir.path(), "logs/a.log.md", "# a");
    write(dir.path(), "logs/b.log.md", "# b");
    write(dir.path(), "logs/important.log.md", "# keep");
    write(dir.path(), ".vaultignore", "*.log.md\n!important.log.md\n");

    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        registered_paths(&vault),
        vec!["logs/important.log.md".to_string()]
    );
}

#[test]
fn no_ignore_keeps_builtins() {
    let (dir, vault) = setup();
    write(dir.path(), "pub.md", "# pub");
    write(dir.path(), "drafts/d.md", "# d");
    write(dir.path(), "node_modules/x.md", "# n");
    write(dir.path(), ".vaultignore", "drafts/\n");

    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            no_ignore: true,
            ..Default::default()
        },
    )
    .unwrap();

    // .vaultignore bypassed → drafts/d.md included
    // Built-ins still apply → node_modules/x.md excluded
    assert_eq!(
        registered_paths(&vault),
        vec!["drafts/d.md".to_string(), "pub.md".to_string()]
    );
}

#[test]
fn force_bypasses_everything() {
    let (dir, vault) = setup();
    write(dir.path(), "pub.md", "# pub");
    write(dir.path(), "drafts/d.md", "# d");
    write(dir.path(), "node_modules/x.md", "# n");
    write(dir.path(), ".vaultignore", "drafts/\n");

    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            force: true,
            ..Default::default()
        },
    )
    .unwrap();

    let paths = registered_paths(&vault);
    assert!(paths.contains(&"node_modules/x.md".to_string()));
    assert!(paths.contains(&"drafts/d.md".to_string()));
    assert!(paths.contains(&"pub.md".to_string()));
}

#[test]
fn vault_state_dir_never_walked() {
    let (dir, vault) = setup();
    write(dir.path(), "real.md", "# r");
    add_paths(
        &vault,
        &[dir.path().to_path_buf()],
        &AddOptions {
            skip_unsupported: true,
            ..Default::default()
        },
    )
    .unwrap();
    let paths = registered_paths(&vault);
    assert!(
        !paths.iter().any(|p| p.starts_with(".vault/")),
        "leaked: {:?}",
        paths
    );
}
