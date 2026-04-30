use std::process::Command;

pub fn is_pid_running(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_pid_running_invalid_pid() {
        // PID 0 is invalid, should return false
        assert!(!is_pid_running(0));
    }

    #[test]
    fn test_is_pid_running_nonexistent_pid() {
        // Use a very high PID that is unlikely to exist
        assert!(!is_pid_running(999999));
    }

    #[test]
    fn test_is_pid_running_current_process() {
        // Get current process ID and verify it's running
        let current_pid = std::process::id();
        assert!(is_pid_running(current_pid));
    }
}
