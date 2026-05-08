use crate::error::Result;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::Path;

pub const BUILT_IN_DEFAULTS: &[&str] = &[
    ".vault/",
    ".vaultignore",
    ".git/",
    ".svn/",
    ".hg/",
    ".gitignore",
    "node_modules/",
    "__pycache__/",
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    "*.pyc",
    ".idea/",
    ".vscode/",
];

pub struct VaultIgnore {
    pub matcher: Gitignore,
    pub respects_vaultignore: bool,
}

impl VaultIgnore {
    /// Build the matcher: built-in defaults plus optional .vaultignore from
    /// `vault_root`. With `respect_vaultignore = false` we use only the
    /// built-in patterns.
    pub fn load(vault_root: &Path, respect_vaultignore: bool) -> Result<Self> {
        let mut b = GitignoreBuilder::new(vault_root);
        for pat in BUILT_IN_DEFAULTS {
            // Errors here are programmer errors (bad patterns); unwrap is OK.
            b.add_line(None, pat).expect("built-in pattern parses");
        }
        if respect_vaultignore {
            let p = vault_root.join(".vaultignore");
            if p.is_file() {
                let _ = b.add(&p);
            }
        }
        let matcher = b
            .build()
            .map_err(|e| crate::error::Error::Other(e.to_string()))?;
        Ok(Self {
            matcher,
            respects_vaultignore: respect_vaultignore,
        })
    }

    /// Empty matcher — used by `rag add --force`.
    pub fn empty(vault_root: &Path) -> Result<Self> {
        let b = GitignoreBuilder::new(vault_root);
        let matcher = b
            .build()
            .map_err(|e| crate::error::Error::Other(e.to_string()))?;
        Ok(Self {
            matcher,
            respects_vaultignore: false,
        })
    }

    /// Defaults-only matcher — used by `rag add --no-ignore`.
    pub fn defaults_only(vault_root: &Path) -> Result<Self> {
        let mut b = GitignoreBuilder::new(vault_root);
        for pat in BUILT_IN_DEFAULTS {
            b.add_line(None, pat).expect("built-in pattern parses");
        }
        let matcher = b
            .build()
            .map_err(|e| crate::error::Error::Other(e.to_string()))?;
        Ok(Self {
            matcher,
            respects_vaultignore: false,
        })
    }

    /// Returns true if the path should be excluded.
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        matches!(
            self.matcher.matched_path_or_any_parents(path, is_dir),
            ignore::Match::Ignore(_)
        )
    }
}
