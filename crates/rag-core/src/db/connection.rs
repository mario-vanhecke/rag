use crate::error::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Once;

static SQLITE_VEC_INIT: Once = Once::new();

fn ensure_sqlite_vec_extension() {
    SQLITE_VEC_INIT.call_once(|| {
        // Register sqlite-vec as a SQLite auto-extension so every connection
        // opened thereafter in this process gets it loaded. `sqlite3_vec_init`
        // is declared as `unsafe extern "C" fn()` in the binding; the real
        // signature matches what `sqlite3_auto_extension` expects, so we
        // transmute the function pointer at the call site.
        type AutoExtFn = unsafe extern "C" fn(
            *mut rusqlite::ffi::sqlite3,
            *mut *const std::os::raw::c_char,
            *const rusqlite::ffi::sqlite3_api_routines,
        ) -> std::os::raw::c_int;
        let init: unsafe extern "C" fn() = sqlite_vec::sqlite3_vec_init;
        let init: AutoExtFn =
            unsafe { std::mem::transmute::<unsafe extern "C" fn(), AutoExtFn>(init) };
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(init));
        }
    });
}

/// Open (or create) a SQLite database at `path` with the conventions `rag` requires:
///   * foreign keys ON
///   * WAL journal mode
///   * busy_timeout = 5000ms
///   * sqlite-vec extension loaded
pub fn open_connection<P: AsRef<Path>>(path: P) -> Result<Connection> {
    ensure_sqlite_vec_extension();
    let conn = Connection::open(path.as_ref())?;
    configure(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    ensure_sqlite_vec_extension();
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(std::time::Duration::from_millis(5000))?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )?;
    Ok(())
}
