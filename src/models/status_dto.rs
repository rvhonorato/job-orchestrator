use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum Status {
    Pending,
    Processing,
    Completed,
    Failed,
    Invalid,
    Queued,
    Submitted,
    Unknown,
    Cleaned,
    Prepared,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Pending => write!(f, "pending"),
            Status::Prepared => write!(f, "prepared"),
            Status::Processing => write!(f, "processing"),
            Status::Completed => write!(f, "completed"),
            Status::Failed => write!(f, "failed"),
            Status::Invalid => write!(f, "invalid"),
            Status::Queued => write!(f, "queued"),
            Status::Submitted => write!(f, "submitted"),
            Status::Unknown => write!(f, "unknown"),
            Status::Cleaned => write!(f, "cleaned"),
        }
    }
}

impl Status {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pending" => Status::Pending,
            "processing" => Status::Processing,
            "completed" => Status::Completed,
            "failed" => Status::Failed,
            "invalid" => Status::Invalid,
            "queued" => Status::Queued,
            "submitted" => Status::Submitted,
            "cleaned" => Status::Cleaned,
            "prepared" => Status::Prepared,
            _ => Status::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Display trait tests =====

    #[test]
    fn test_display_pending() {
        assert_eq!(format!("{}", Status::Pending), "pending");
    }

    #[test]
    fn test_display_prepared() {
        assert_eq!(format!("{}", Status::Prepared), "prepared");
    }

    #[test]
    fn test_display_processing() {
        assert_eq!(format!("{}", Status::Processing), "processing");
    }

    #[test]
    fn test_display_completed() {
        assert_eq!(format!("{}", Status::Completed), "completed");
    }

    #[test]
    fn test_display_failed() {
        assert_eq!(format!("{}", Status::Failed), "failed");
    }

    #[test]
    fn test_display_invalid() {
        assert_eq!(format!("{}", Status::Invalid), "invalid");
    }

    #[test]
    fn test_display_queued() {
        assert_eq!(format!("{}", Status::Queued), "queued");
    }

    #[test]
    fn test_display_submitted() {
        assert_eq!(format!("{}", Status::Submitted), "submitted");
    }

    #[test]
    fn test_display_unknown() {
        assert_eq!(format!("{}", Status::Unknown), "unknown");
    }

    #[test]
    fn test_display_cleaned() {
        assert_eq!(format!("{}", Status::Cleaned), "cleaned");
    }

    // ===== from_string tests =====

    #[test]
    fn test_from_string_lowercase() {
        assert_eq!(Status::from_string("pending"), Status::Pending);
        assert_eq!(Status::from_string("processing"), Status::Processing);
        assert_eq!(Status::from_string("completed"), Status::Completed);
        assert_eq!(Status::from_string("failed"), Status::Failed);
        assert_eq!(Status::from_string("invalid"), Status::Invalid);
        assert_eq!(Status::from_string("queued"), Status::Queued);
        assert_eq!(Status::from_string("submitted"), Status::Submitted);
        assert_eq!(Status::from_string("cleaned"), Status::Cleaned);
        assert_eq!(Status::from_string("prepared"), Status::Prepared);
    }

    #[test]
    fn test_from_string_uppercase() {
        assert_eq!(Status::from_string("PENDING"), Status::Pending);
        assert_eq!(Status::from_string("PROCESSING"), Status::Processing);
        assert_eq!(Status::from_string("COMPLETED"), Status::Completed);
        assert_eq!(Status::from_string("FAILED"), Status::Failed);
        assert_eq!(Status::from_string("INVALID"), Status::Invalid);
        assert_eq!(Status::from_string("QUEUED"), Status::Queued);
        assert_eq!(Status::from_string("SUBMITTED"), Status::Submitted);
        assert_eq!(Status::from_string("CLEANED"), Status::Cleaned);
        assert_eq!(Status::from_string("PREPARED"), Status::Prepared);
    }

    #[test]
    fn test_from_string_mixed_case() {
        assert_eq!(Status::from_string("PeNdInG"), Status::Pending);
        assert_eq!(Status::from_string("ProCeSsiNG"), Status::Processing);
        assert_eq!(Status::from_string("ComPlEtEd"), Status::Completed);
    }

    #[test]
    fn test_from_string_unrecognized() {
        assert_eq!(Status::from_string("notastatus"), Status::Unknown);
        assert_eq!(Status::from_string("random"), Status::Unknown);
        assert_eq!(Status::from_string("xyz"), Status::Unknown);
    }

    #[test]
    fn test_from_string_empty() {
        assert_eq!(Status::from_string(""), Status::Unknown);
    }

    #[test]
    fn test_from_string_whitespace() {
        assert_eq!(Status::from_string("  "), Status::Unknown);
        assert_eq!(Status::from_string("\t"), Status::Unknown);
        assert_eq!(Status::from_string("\n"), Status::Unknown);
    }

    #[test]
    fn test_from_string_with_whitespace() {
        // Note: from_string doesn't trim whitespace
        assert_eq!(Status::from_string(" pending "), Status::Unknown);
        assert_eq!(Status::from_string("pending "), Status::Unknown);
    }

    #[test]
    fn test_from_string_prepared() {
        assert_eq!(Status::from_string("prepared"), Status::Prepared);
    }

    // ===== Round-trip tests =====

    #[test]
    fn test_roundtrip_display_from_string() {
        // Test that Display -> from_string works for statuses that have from_string support
        assert_eq!(
            Status::from_string(&format!("{}", Status::Pending)),
            Status::Pending
        );
        assert_eq!(
            Status::from_string(&format!("{}", Status::Processing)),
            Status::Processing
        );
        assert_eq!(
            Status::from_string(&format!("{}", Status::Completed)),
            Status::Completed
        );
        assert_eq!(
            Status::from_string(&format!("{}", Status::Failed)),
            Status::Failed
        );
        assert_eq!(
            Status::from_string(&format!("{}", Status::Invalid)),
            Status::Invalid
        );
        assert_eq!(
            Status::from_string(&format!("{}", Status::Queued)),
            Status::Queued
        );
        assert_eq!(
            Status::from_string(&format!("{}", Status::Submitted)),
            Status::Submitted
        );
        assert_eq!(
            Status::from_string(&format!("{}", Status::Cleaned)),
            Status::Cleaned
        );
        assert_eq!(
            Status::from_string(&format!("{}", Status::Prepared)),
            Status::Prepared
        );
    }

    // ===== Equality tests =====

    #[test]
    fn test_status_equality() {
        assert_eq!(Status::Pending, Status::Pending);
        assert_ne!(Status::Pending, Status::Processing);
        assert_ne!(Status::Completed, Status::Failed);
    }

    #[test]
    fn test_status_clone() {
        let status = Status::Pending;
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }
}
