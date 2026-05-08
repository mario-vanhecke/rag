use rag_core::Error;

#[allow(dead_code)]
pub const SUCCESS: i32 = 0;
pub const GENERAL: i32 = 1;
#[allow(dead_code)]
pub const INVALID_USAGE: i32 = 2;
pub const NO_VAULT: i32 = 3;
pub const VAULT_CORRUPTION: i32 = 4;
pub const CONFIG_ERROR: i32 = 5;
pub const IO_ERROR: i32 = 6;
pub const LOCK_CONTENTION: i32 = 7;
pub const SUBPROCESS_ERROR: i32 = 8;

pub fn for_error(e: &Error) -> i32 {
    match e {
        Error::NoVault { .. } => NO_VAULT,
        Error::SchemaMismatch { .. } => VAULT_CORRUPTION,
        Error::Config(_) => CONFIG_ERROR,
        Error::Io(_) => IO_ERROR,
        Error::LockContention => LOCK_CONTENTION,
        Error::Subprocess(_) => SUBPROCESS_ERROR,
        Error::Invariant(_) => VAULT_CORRUPTION,
        _ => GENERAL,
    }
}
