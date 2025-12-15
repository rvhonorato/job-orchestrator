use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Health {
    pub status: String,
    pub database: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_creation() {
        let health = Health {
            status: "healthy".to_string(),
            database: "connected".to_string(),
        };

        assert_eq!(health.status, "healthy");
        assert_eq!(health.database, "connected");
    }

    #[test]
    fn test_health_serialization() {
        let health = Health {
            status: "healthy".to_string(),
            database: "connected".to_string(),
        };

        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"database\":\"connected\""));
    }

    #[test]
    fn test_health_deserialization() {
        let json = r#"{"status":"healthy","database":"connected"}"#;
        let health: Health = serde_json::from_str(json).unwrap();

        assert_eq!(health.status, "healthy");
        assert_eq!(health.database, "connected");
    }

    #[test]
    fn test_health_roundtrip() {
        let original = Health {
            status: "degraded".to_string(),
            database: "disconnected".to_string(),
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Health = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.status, original.status);
        assert_eq!(deserialized.database, original.database);
    }
}
