//! Best-effort owner-only file permission restriction (Unix only).
//!
//! The database and log files hold transcript content, so on a
//! multi-user machine they shouldn't be left at the default umask
//! (typically 0644/world-readable) — see #188.

use std::path::Path;

/// Restrict `path` to owner-only access at the given octal `mode`
/// (e.g. `0o600` for a file, `0o700` for a directory). Best-effort: a
/// failure (e.g. an unsupported filesystem) is logged and otherwise
/// ignored — this is defense in depth, not a correctness requirement,
/// so it must never make startup fail.
#[cfg(unix)]
pub fn restrict_to_owner(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)) {
        tracing::warn!("failed to restrict permissions on {}: {e}", path.display());
    }
}

#[cfg(not(unix))]
pub fn restrict_to_owner(_path: &Path, _mode: u32) {
    // No-op on non-Unix platforms — Windows ACLs work differently and
    // aren't covered by this fix (see #188's discussion).
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn restrict_to_owner_sets_the_requested_mode_on_a_file() {
        let test_name = format!("polyvocal-fsperm-test-{}", uuid::Uuid::new_v4());
        let dir = std::env::temp_dir().join(test_name);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.txt");
        std::fs::write(&file, b"hello").unwrap();

        restrict_to_owner(&file, 0o600);

        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restrict_to_owner_sets_the_requested_mode_on_a_directory() {
        let test_name = format!("polyvocal-fsperm-test-{}", uuid::Uuid::new_v4());
        let dir = std::env::temp_dir().join(test_name);
        std::fs::create_dir_all(&dir).unwrap();

        restrict_to_owner(&dir, 0o700);

        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
