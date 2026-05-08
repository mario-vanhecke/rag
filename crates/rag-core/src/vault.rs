use crate::config::Config;
use crate::db::{connection::open_connection, migrations};
use crate::error::{Error, Result};
use rusqlite::{params, Connection};
use std::path::{Component, Path, PathBuf};

pub const STATE_DIR: &str = ".vault";
pub const DB_FILE: &str = "vault";
pub const CACHE_DIR: &str = "cache";
pub const LOGS_DIR: &str = "logs";
pub const MODELS_DIR: &str = "models";
pub const INDEX_LOCK: &str = "index.lock";

pub struct Vault {
    pub root: PathBuf,
    pub state_dir: PathBuf,
    pub db_path: PathBuf,
    pub conn: Connection,
    pub config: Config,
}

impl Vault {
    /// Walk up from `start` to find a `.vault/` directory, then open the vault.
    pub fn discover(start: &Path) -> Result<Self> {
        let start = if start.is_absolute() {
            start.to_path_buf()
        } else {
            std::env::current_dir()?.join(start)
        };
        let mut cur = start.as_path();
        loop {
            let candidate = cur.join(STATE_DIR);
            if candidate.is_dir() {
                return Self::open(cur);
            }
            match cur.parent() {
                Some(p) => cur = p,
                None => {
                    return Err(Error::NoVault {
                        start: start.clone(),
                    });
                }
            }
        }
    }

    /// Open an existing vault rooted at `vault_root` (the parent of `.vault/`).
    pub fn open(vault_root: &Path) -> Result<Self> {
        let root = vault_root.canonicalize()?;
        let state_dir = root.join(STATE_DIR);
        if !state_dir.is_dir() {
            return Err(Error::NoVault {
                start: root.clone(),
            });
        }
        let db_path = state_dir.join(DB_FILE);
        let mut conn = open_connection(&db_path)?;
        migrations::apply_pending(&mut conn)?;
        let config = Config::load(&conn)?;
        Ok(Self {
            root,
            state_dir,
            db_path,
            conn,
            config,
        })
    }

    /// Create a new vault at `vault_root`.
    pub fn init(vault_root: &Path, force: bool) -> Result<Self> {
        std::fs::create_dir_all(vault_root)?;
        let root = vault_root.canonicalize()?;
        let state_dir = root.join(STATE_DIR);

        if state_dir.exists() && !force {
            return Err(Error::VaultExists {
                path: state_dir.clone(),
            });
        }
        std::fs::create_dir_all(&state_dir)?;
        std::fs::create_dir_all(state_dir.join(CACHE_DIR).join(MODELS_DIR))?;
        std::fs::create_dir_all(state_dir.join(LOGS_DIR))?;

        let db_path = state_dir.join(DB_FILE);
        let mut conn = open_connection(&db_path)?;
        migrations::apply_pending(&mut conn)?;

        let now = chrono::Utc::now().timestamp_millis();
        let vault_id = uuid::Uuid::now_v7().to_string();
        let tool_version = env!("CARGO_PKG_VERSION");
        // INSERT OR IGNORE so re-init with --force on an existing db is safe.
        conn.execute(
            "INSERT OR IGNORE INTO vault_meta (id, vault_id, created_at, tool_version)
             VALUES (1, ?1, ?2, ?3)",
            params![vault_id, now, tool_version],
        )?;

        let config = Config::load(&conn)?;
        Ok(Self {
            root,
            state_dir,
            db_path,
            conn,
            config,
        })
    }

    /// Return (vault_id, created_at, tool_version) from vault_meta.
    pub fn meta(&self) -> Result<(String, i64, String)> {
        let row = self.conn.query_row(
            "SELECT vault_id, created_at, tool_version FROM vault_meta WHERE id = 1",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )?;
        Ok(row)
    }

    pub fn schema_version(&self) -> Result<u32> {
        migrations::current_version(&self.conn)
    }

    pub fn index_lock_path(&self) -> PathBuf {
        self.state_dir.join(INDEX_LOCK)
    }

    pub fn models_dir(&self) -> PathBuf {
        self.state_dir.join(CACHE_DIR).join(MODELS_DIR)
    }

    /// Convert any path (absolute or relative to cwd) into a vault-root-relative
    /// path with forward slashes. Errors if the path escapes the vault.
    pub fn relativize(&self, path: &Path) -> Result<PathBuf> {
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        // We don't require the path to exist (e.g. for `rag rm`).
        let abs = match abs.canonicalize() {
            Ok(p) => p,
            Err(_) => normalize_logical(&abs),
        };
        let rel = abs.strip_prefix(&self.root).map_err(|_| {
            Error::InvalidPath(format!(
                "{} is outside vault {}",
                abs.display(),
                self.root.display()
            ))
        })?;
        Ok(to_forward_slashes(rel))
    }

    pub fn absolutize(&self, rel_path: &str) -> PathBuf {
        let mut p = self.root.clone();
        for comp in rel_path.split('/') {
            if comp.is_empty() || comp == "." {
                continue;
            }
            p.push(comp);
        }
        p
    }
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
