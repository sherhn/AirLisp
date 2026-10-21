//! UI independent core of the IDE: editor model, command registry, project model and search.

pub mod commands;
pub mod diff;
pub mod editor;
pub mod exec;
pub mod git;
pub mod index;
pub mod localvcs;
pub mod search;
pub mod settings;
pub mod workspace;

#[cfg(test)]
pub(crate) mod testutil {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A uniquely named temp directory, removed on drop.
    pub struct TempDir(PathBuf);

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    impl TempDir {
        pub fn new(tag: &str) -> TempDir {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let p = std::env::temp_dir().join(format!("lispcraft-test-{}-{}-{}", std::process::id(), tag, n));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
