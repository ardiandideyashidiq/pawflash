//! Permission-checking helpers for serial port access.

/// Returns `true` if the error message indicates a permission-denied error.
///
/// Works by checking if any error in the source chain matches platform-specific
/// permission error patterns.
#[must_use]
pub fn is_permission_error(err: &dyn std::error::Error) -> bool {
    let patterns = crate::platform::CURRENT.permission_error_patterns();
    let mut current: Option<&dyn std::error::Error> = Some(err);
    while let Some(e) = current {
        let msg = e.to_string().to_lowercase();
        if patterns.iter().any(|p| msg.contains(p)) {
            return true;
        }
        current = e.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_denied_via_message() {
        let err = std::io::Error::other("Permission denied");
        assert!(is_permission_error(&err));
    }

    #[test]
    fn not_permission_error() {
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        assert!(!is_permission_error(&err));
    }

    #[test]
    fn empty_message() {
        let err = std::io::Error::other("");
        assert!(!is_permission_error(&err));
    }
}
