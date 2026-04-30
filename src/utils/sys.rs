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
    fn test_is_pid_running_nonexistent_pid() {
        // Use a very high PID that is unlikely to exist
        // Note: PID 0 may behave differently on different systems, so we skip it
        assert!(!is_pid_running(999999));
        assert!(!is_pid_running(999998));
        assert!(!is_pid_running(1 << 20)); // Very high PID unlikely to be in use
    }

    #[test]
    fn test_is_pid_running_current_process() {
        // Get current process ID and verify it's running
        let current_pid = std::process::id();
        assert!(is_pid_running(current_pid));
    }
}
