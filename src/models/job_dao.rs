use crate::models::status_dto::Status;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(serde::Serialize, Debug, ToSchema)]
pub struct Job {
    pub id: u32,
    pub user_id: i32,
    pub service: String,
    pub status: Status,
    #[schema(value_type = String)]
    pub loc: PathBuf,
    pub dest_id: u32,
    pub pid: u32,
}

impl Job {
    pub fn new(data_path: &str) -> Job {
        let loc = std::path::Path::new(&data_path).join(Uuid::new_v4().to_string());
        Job {
            id: 0,
            user_id: 0,
            service: String::new(),
            status: Status::Unknown,
            loc,
            dest_id: 0,
            pid: 0,
        }
    }

    pub fn download(self) -> Result<Vec<u8>, std::io::Error> {
        let mut file = fs::File::open(self.loc.join("output.zip"))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    pub fn remove_from_disk(&self) -> Result<(), std::io::Error> {
        fs::remove_dir_all(&self.loc)
    }

    pub fn set_service(&mut self, service: String) {
        self.service = service
    }

    pub fn set_user_id(&mut self, user_id: i32) {
        self.user_id = user_id;
    }

    pub fn get_status(&self) -> Status {
        self.status
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_download() {
        let tempdir = TempDir::new().unwrap();
        let job = Job::new(tempdir.path().to_str().unwrap());

        let _ = fs::create_dir_all(&job.loc);
        let test_data = b"test content".to_vec();
        fs::write(job.loc.join("output.zip"), &test_data).unwrap();

        let result = job.download().unwrap();
        assert_eq!(result, test_data);
    }

    #[test]
    fn test_remove_from_disk() {
        let tempdir = TempDir::new().unwrap();
        let job = Job::new(tempdir.path().to_str().unwrap());

        // First verify the directory exists
        fs::create_dir_all(&job.loc).unwrap();
        assert!(Path::new(&job.loc).exists());

        // Remove the directory
        let _ = job.remove_from_disk();

        // Verify the directory no longer exists
        assert!(!Path::new(&job.loc).exists());
    }

    #[test]
    fn test_set_service() {
        let mut job = Job::new("");
        job.set_service("test".to_string());
        assert_eq!(job.service, "test".to_string())
    }

    #[test]
    fn test_set_user_id() {
        let mut job = Job::new("");
        job.set_user_id(99);
        assert_eq!(job.user_id, 99)
    }
}
