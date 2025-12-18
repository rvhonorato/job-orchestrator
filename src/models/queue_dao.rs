use super::job_dao::Job;
use crate::{config::loader::Config, models::payload_dao::Payload};

#[derive(Debug)]
pub struct Queue<'a> {
    pub jobs: Vec<Job>,
    pub config: &'a Config,
}

impl Queue<'_> {
    pub fn new(config: &Config) -> Queue<'_> {
        Queue {
            jobs: Vec::new(),
            config,
        }
    }
}

#[derive(Debug)]
pub struct PayloadQueue<'a> {
    pub jobs: Vec<Payload>,
    pub config: &'a Config,
}

impl PayloadQueue<'_> {
    pub fn new(config: &Config) -> PayloadQueue<'_> {
        PayloadQueue {
            jobs: Vec::new(),
            config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::Config;
    use std::collections::HashMap;
    use std::time::Duration;

    fn create_test_config() -> Config {
        Config {
            services: HashMap::new(),
            db_path: "/test/db.sqlite".to_string(),
            data_path: "/test/data".to_string(),
            max_age: Duration::from_secs(3600),
            port: 1111,
        }
    }

    // ===== Queue tests =====

    #[test]
    fn test_queue_new() {
        let config = create_test_config();
        let queue = Queue::new(&config);

        assert_eq!(queue.jobs.len(), 0);
        assert_eq!(queue.config.db_path, "/test/db.sqlite");
    }

    #[test]
    fn test_queue_new_empty_jobs() {
        let config = create_test_config();
        let queue = Queue::new(&config);

        assert!(queue.jobs.is_empty());
    }

    #[test]
    fn test_queue_references_config() {
        let config = create_test_config();
        let queue = Queue::new(&config);

        // Verify that the queue references the same config
        assert_eq!(queue.config.data_path, config.data_path);
        assert_eq!(queue.config.db_path, config.db_path);
        assert_eq!(queue.config.max_age, config.max_age);
    }

    // ===== PayloadQueue tests =====

    #[test]
    fn test_payload_queue_new() {
        let config = create_test_config();
        let queue = PayloadQueue::new(&config);

        assert_eq!(queue.jobs.len(), 0);
        assert_eq!(queue.config.db_path, "/test/db.sqlite");
    }

    #[test]
    fn test_payload_queue_new_empty_jobs() {
        let config = create_test_config();
        let queue = PayloadQueue::new(&config);

        assert!(queue.jobs.is_empty());
    }

    #[test]
    fn test_payload_queue_references_config() {
        let config = create_test_config();
        let queue = PayloadQueue::new(&config);

        // Verify that the queue references the same config
        assert_eq!(queue.config.data_path, config.data_path);
        assert_eq!(queue.config.db_path, config.db_path);
        assert_eq!(queue.config.max_age, config.max_age);
    }

    #[test]
    fn test_multiple_queues_same_config() {
        let config = create_test_config();
        let queue1 = Queue::new(&config);
        let queue2 = Queue::new(&config);

        // Both queues should reference the same config
        assert_eq!(queue1.config.db_path, queue2.config.db_path);
    }

    #[test]
    fn test_multiple_payload_queues_same_config() {
        let config = create_test_config();
        let queue1 = PayloadQueue::new(&config);
        let queue2 = PayloadQueue::new(&config);

        // Both queues should reference the same config
        assert_eq!(queue1.config.db_path, queue2.config.db_path);
    }
}
