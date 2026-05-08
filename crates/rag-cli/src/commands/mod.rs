pub mod add;
pub mod config;
pub mod index;
pub mod info;
pub mod init;
pub mod ls;
pub mod prune;
pub mod rm;
pub mod search;
pub mod show;
pub mod status;

use rag_core::Vault;
use std::path::Path;

pub fn open_vault(vault_arg: Option<&Path>) -> anyhow::Result<Vault> {
    let v = match vault_arg {
        Some(p) => Vault::open(p)?,
        None => {
            let cwd = std::env::current_dir()?;
            Vault::discover(&cwd)?
        }
    };
    Ok(v)
}
