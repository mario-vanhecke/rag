//! Path helpers tools share for storing vault-relative paths consistently.

use crate::error::{Error, Result};
use std::path::{Component, Path, PathBuf};

/// Walk up from `start` looking for a directory named `state_dir_name`.
/// Returns the *parent* of that directory (i.e., the vault/manifest root).
pub fn discover_state_root(start: &Path, state_dir_name: &str) -> Result<PathBuf> {
    let start = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir()?.join(start)
    };
    let mut cur = start.as_path();
    loop {
        if cur.join(state_dir_name).is_dir() {
            return Ok(cur.to_path_buf());
        }
        match cur.parent() {
            Some(p) => cur = p,
            None => {
                return Err(Error::NoState {
                    name: state_dir_name.to_string(),
                    start: start.clone(),
                });
            }
        }
    }
}

/// Convert a path (absolute or cwd-relative) into a `root`-relative path with
/// forward slashes. Errors if the path escapes `root`. Tolerates non-existent
/// paths (e.g., for `rm` on already-deleted files) by falling back to a
/// logical normalize-then-strip.
pub fn relativize(root: &Path, path: &Path) -> Result<PathBuf> {
    let abs0 = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let abs = match abs0.canonicalize() {
        Ok(p) => p,
        Err(_) => normalize_logical(&abs0),
    };
    let rel = abs.strip_prefix(root).map_err(|_| {
        Error::InvalidPath(format!(
            "{} is outside vault {}",
            abs.display(),
            root.display()
        ))
    })?;
    Ok(to_forward_slashes(rel))
}

/// Convert a vault-relative forward-slash path back to an absolute filesystem
/// path under `root`.
pub fn absolutize(root: &Path, rel_path: &str) -> PathBuf {
    let mut p = root.to_path_buf();
    for comp in rel_path.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        }
        p.push(comp);
    }
    p
}

fn to_forward_slashes(p: &Path) -> PathBuf {
    let mut out = String::new();
    let mut first = true;
    for c in p.components() {
        let s = match c {
            Component::Normal(s) => s.to_string_lossy().into_owned(),
            Component::CurDir => continue,
            Component::ParentDir => "..".to_string(),
            Component::RootDir => continue,
            Component::Prefix(p) => p.as_os_str().to_string_lossy().into_owned(),
        };
        if !first {
            out.push('/');
        }
        out.push_str(&s);
        first = false;
    }
    PathBuf::from(out)
}

fn normalize_logical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
