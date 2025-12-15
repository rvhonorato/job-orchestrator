use serde::Serialize;

#[derive(Serialize)]
pub struct Ping {
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_creation() {
        let ping = Ping {
            message: "pong".to_string(),
        };

        assert_eq!(ping.message, "pong");
    }

    #[test]
    fn test_ping_serialization() {
        let ping = Ping {
            message: "pong".to_string(),
        };

        let json = serde_json::to_string(&ping).unwrap();
        assert!(json.contains("\"message\":\"pong\""));
    }

    #[test]
    fn test_ping_with_different_messages() {
        let ping1 = Ping {
            message: "Hello".to_string(),
        };
        let ping2 = Ping {
            message: "World".to_string(),
        };

        assert_eq!(ping1.message, "Hello");
        assert_eq!(ping2.message, "World");
    }

    #[test]
    fn test_ping_serialization_format() {
        let ping = Ping {
            message: "test message".to_string(),
        };

        let json = serde_json::to_string(&ping).unwrap();
        let expected = r#"{"message":"test message"}"#;
        assert_eq!(json, expected);
    }
}
