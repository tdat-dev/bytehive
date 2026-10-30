//! Build edition: the side-by-side "Maestro Supervisor" build vs regular Maestro.
//!
//! Detected at runtime from the executable's own file name, so it is always
//! right no matter which target dir or config produced the binary:
//! `maestro-supervisor.exe` is the Supervisor edition, anything else is not.
//! (The Cargo binary is always `maestro-dev.exe`; `tauri build` renames it via
//! `mainBinaryName`, and the portable Supervisor build is renamed by hand.)

/// True when `name` (a file stem, no extension) is the Supervisor edition's.
pub fn is_supervisor_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("maestro-supervisor")
}

/// True when this process is the Supervisor edition.
pub fn is_supervisor() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .is_some_and(|n| is_supervisor_name(&n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(is_supervisor_name("maestro-supervisor"));
        assert!(is_supervisor_name("Maestro-Supervisor"));
        assert!(!is_supervisor_name("maestro"));
        assert!(!is_supervisor_name("maestro-dev"));
        assert!(!is_supervisor_name(""));
    }

    #[test]
    fn test_harness_is_not_supervisor() {
        assert!(!is_supervisor());
    }
}
