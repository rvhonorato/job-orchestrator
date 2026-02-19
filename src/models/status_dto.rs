use http::StatusCode;
use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

// TODO: These statuses are a bit confusing, some of them are just
// used in the client and some only in the server and some used in both
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum Status {
    Queued,     // Job recived in the server
    Processing, // Job is being sent to the client
    Submitted,  // Job was sent to the client
    Prepared,   // Ready to be executed in the client
    Running,    // Running in the client
    Cleaned,    // Job has been cleaned
    Completed,  // Job complete
    Failed,     // Job failed
    Invalid,    // Job invalid
    Unknown,    // Wildcard
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Prepared => write!(f, "prepared"),
            Status::Processing => write!(f, "processing"),
            Status::Completed => write!(f, "completed"),
            Status::Failed => write!(f, "failed"),
            Status::Invalid => write!(f, "invalid"),
            Status::Queued => write!(f, "queued"),
            Status::Submitted => write!(f, "submitted"),
            Status::Unknown => write!(f, "unknown"),
            Status::Cleaned => write!(f, "cleaned"),
            Status::Running => write!(f, "running"),
        }
    }
}

impl Status {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "processing" => Status::Processing,
            "completed" => Status::Completed,
            "failed" => Status::Failed,
            "invalid" => Status::Invalid,
            "queued" => Status::Queued,
            "submitted" => Status::Submitted,
            "cleaned" => Status::Cleaned,
            "prepared" => Status::Prepared,
            "running" => Status::Running,
            _ => Status::Unknown,
        }
    }

    pub fn as_http_code(&self) -> http::StatusCode {
        match self {
            Status::Completed => StatusCode::OK,
            Status::Cleaned => StatusCode::NO_CONTENT,
            Status::Failed => StatusCode::GONE,
            Status::Invalid => StatusCode::BAD_REQUEST,
            Status::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
            Status::Submitted | Status::Running => StatusCode::ACCEPTED,
            Status::Processing | Status::Queued | Status::Prepared => StatusCode::CREATED,
        }
    }

    // pub fn description(&self) -> &'static str {
    //     match self {
    //         Status::Queued => "Job received in the server",
    //         Status::Processing => "Job is being sent to the client",
    //         Status::Submitted => "Job was sent to the client",
    //         Status::Prepared => "Ready to be executed in the client",
    //         Status::Running => "Running in the client",
    //         Status::Cleaned => "Job has been cleaned",
    //         Status::Completed => "Job complete",
    //         Status::Failed => "Job failed",
    //         Status::Invalid => "Job invalid",
    //         Status::Unknown => "Wildcard",
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Display trait tests =====

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

    #[test]
    fn test_display_running() {
        assert_eq!(format!("{}", Status::Running), "running");
    }

    // ===== from_string tests =====

    #[test]
    fn test_from_string_lowercase() {
        assert_eq!(Status::from_string("processing"), Status::Processing);
        assert_eq!(Status::from_string("completed"), Status::Completed);
        assert_eq!(Status::from_string("failed"), Status::Failed);
        assert_eq!(Status::from_string("invalid"), Status::Invalid);
        assert_eq!(Status::from_string("queued"), Status::Queued);
        assert_eq!(Status::from_string("submitted"), Status::Submitted);
        assert_eq!(Status::from_string("cleaned"), Status::Cleaned);
        assert_eq!(Status::from_string("prepared"), Status::Prepared);
        assert_eq!(Status::from_string("running"), Status::Running);
    }

    #[test]
    fn test_from_string_uppercase() {
        assert_eq!(Status::from_string("PROCESSING"), Status::Processing);
        assert_eq!(Status::from_string("COMPLETED"), Status::Completed);
        assert_eq!(Status::from_string("FAILED"), Status::Failed);
        assert_eq!(Status::from_string("INVALID"), Status::Invalid);
        assert_eq!(Status::from_string("QUEUED"), Status::Queued);
        assert_eq!(Status::from_string("SUBMITTED"), Status::Submitted);
        assert_eq!(Status::from_string("CLEANED"), Status::Cleaned);
        assert_eq!(Status::from_string("PREPARED"), Status::Prepared);
        assert_eq!(Status::from_string("Running"), Status::Running);
    }

    #[test]
    fn test_from_string_mixed_case() {
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
        assert_eq!(
            Status::from_string(&format!("{}", Status::Running)),
            Status::Running
        );
    }

    // ===== Equality tests =====

    #[test]
    fn test_status_equality() {
        assert_eq!(Status::Completed, Status::Completed);
        assert_ne!(Status::Queued, Status::Processing);
        assert_ne!(Status::Completed, Status::Failed);
    }

    #[test]
    fn test_status_clone() {
        let status = Status::Processing;
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }
}
