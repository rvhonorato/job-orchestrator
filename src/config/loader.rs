use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;
use std::{env, time};
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub services: HashMap<String, Service>,
    pub db_path: String,
    pub data_path: String,
    pub max_age: Duration,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Service {
    pub name: String,
    pub upload_url: String,
    pub download_url: String,
    pub runs_per_user: u16,
}

impl Config {
    pub fn new() -> Result<Config, Box<dyn Error>> {
        let mut services = HashMap::new();

        // Iterate over all environment variables
        for (key, value) in env::vars() {
            // Look for service environment variables with the pattern:
            // - SERVICE_<NAME>_UPLOAD_URL
            // - SERVICE_<NAME>_DOWNLOAD_URL
            // - SERVICE_<NAME>_RUNS_PER_USER
            if key.starts_with("SERVICE_") {
                let parts: Vec<&str> = key.split('_').collect();
                if parts.len() >= 3 {
                    let service_name = parts[1]; // Extract the service name from the variable
                    let service_vars = parts[2..].join("_"); // Join the rest for type

                    // Use the service name as a key to store the service info
                    let service = services
                        .entry(service_name.to_string().to_ascii_lowercase())
                        .or_insert(Service {
                            name: service_name.to_string().to_ascii_lowercase(),
                            upload_url: String::new(),
                            download_url: String::new(),
                            runs_per_user: 5, // by default consider 5 runs per user per service
                        });

                    // Assign the corresponding vars to the config
                    match service_vars.as_str() {
                        "UPLOAD_URL" => service.upload_url = value,
                        "DOWNLOAD_URL" => service.download_url = value,
                        "RUNS_PER_USER" => service.runs_per_user = value.parse::<u16>().unwrap(),
                        _ => continue,
                    };
                }
            }
        }

        let wd = env::current_dir().unwrap().display().to_string();

        let db_path = match env::var("DB_PATH") {
            Ok(p) => p,
            Err(_) => {
                let db_path = format!("{}/db.sqlite", wd.clone());
                warn!("DB_PATH not defined, using {:?}", db_path);
                db_path
            }
        };

        let data_path = match env::var("DATA_PATH") {
            Ok(p) => p,
            Err(_) => {
                let data_path = format!("{}/data", wd);
                warn!("DATA_PATH not defined, using {:?}", data_path);
                data_path
            }
        };

        let max_age = match env::var("MAX_AGE") {
            Ok(v) => {
                let time: u64 = v.parse().unwrap();
                time::Duration::from_secs(time)
            }
            Err(_) => {
                let duration = time::Duration::from_secs(864000);
                warn!("MAX_AGE not defined, using {:?}", duration);
                duration
            }
        };

        let config = Config {
            services,
            db_path,
            data_path,
            max_age,
        };
        info!("{:?}", config);
        Ok(config)
    }

    pub fn get_download_url(&self, service_name: &str) -> Option<&str> {
        self.services
            .get(service_name)
            .map(|service| service.download_url.as_str())
    }

    pub fn get_upload_url(&self, service_name: &str) -> Option<&str> {
        self.services
            .get(service_name)
            .map(|service| service.upload_url.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Helper to create a test config manually
    fn create_test_config() -> Config {
        let mut services = HashMap::new();

        services.insert(
            "test".to_string(),
            Service {
                name: "test".to_string(),
                upload_url: "http://test.com/upload".to_string(),
                download_url: "http://test.com/download".to_string(),
                runs_per_user: 10,
            },
        );

        Config {
            services,
            db_path: "/test/db.sqlite".to_string(),
            data_path: "/test/data".to_string(),
            max_age: Duration::from_secs(3600),
        }
    }

    // ===== Service structure tests =====

    #[test]
    fn test_service_creation() {
        let service = Service {
            name: "test".to_string(),
            upload_url: "http://example.com/upload".to_string(),
            download_url: "http://example.com/download".to_string(),
            runs_per_user: 5,
        };

        assert_eq!(service.name, "test");
        assert_eq!(service.upload_url, "http://example.com/upload");
        assert_eq!(service.download_url, "http://example.com/download");
        assert_eq!(service.runs_per_user, 5);
    }

    // ===== get_download_url tests =====

    #[test]
    fn test_get_download_url_existing_service() {
        let config = create_test_config();
        let url = config.get_download_url("test");

        assert_eq!(url, Some("http://test.com/download"));
    }

    #[test]
    fn test_get_download_url_nonexistent_service() {
        let config = create_test_config();
        let url = config.get_download_url("nonexistent");

        assert_eq!(url, None);
    }

    #[test]
    fn test_get_download_url_empty_service_name() {
        let config = create_test_config();
        let url = config.get_download_url("");

        assert_eq!(url, None);
    }

    // ===== get_upload_url tests =====

    #[test]
    fn test_get_upload_url_existing_service() {
        let config = create_test_config();
        let url = config.get_upload_url("test");

        assert_eq!(url, Some("http://test.com/upload"));
    }

    #[test]
    fn test_get_upload_url_nonexistent_service() {
        let config = create_test_config();
        let url = config.get_upload_url("nonexistent");

        assert_eq!(url, None);
    }

    #[test]
    fn test_get_upload_url_empty_service_name() {
        let config = create_test_config();
        let url = config.get_upload_url("");

        assert_eq!(url, None);
    }

    // ===== Config structure tests =====

    #[test]
    fn test_config_structure() {
        let config = create_test_config();

        assert_eq!(config.services.len(), 1);
        assert_eq!(config.db_path, "/test/db.sqlite");
        assert_eq!(config.data_path, "/test/data");
        assert_eq!(config.max_age, Duration::from_secs(3600));
    }

    #[test]
    fn test_config_multiple_services() {
        let mut services = HashMap::new();

        services.insert(
            "service1".to_string(),
            Service {
                name: "service1".to_string(),
                upload_url: "http://s1.com/upload".to_string(),
                download_url: "http://s1.com/download".to_string(),
                runs_per_user: 5,
            },
        );

        services.insert(
            "service2".to_string(),
            Service {
                name: "service2".to_string(),
                upload_url: "http://s2.com/upload".to_string(),
                download_url: "http://s2.com/download".to_string(),
                runs_per_user: 10,
            },
        );

        let config = Config {
            services,
            db_path: "/test/db.sqlite".to_string(),
            data_path: "/test/data".to_string(),
            max_age: Duration::from_secs(7200),
        };

        assert_eq!(config.services.len(), 2);
        assert!(config.get_upload_url("service1").is_some());
        assert!(config.get_upload_url("service2").is_some());
    }
}
