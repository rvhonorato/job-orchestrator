use crate::models::status_dto::Status;
use crate::utils;
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
        }
    }

    pub fn download(self) -> Result<Vec<u8>, std::io::Error> {
        let mut file = fs::File::open(self.loc.join("output.zip"))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    /// Zip the job directory to bytes, regardless of its current state.
    /// This is used for partial downloads to debug stuck or incomplete runs.
    pub fn download_partial(self) -> Result<Vec<u8>, std::io::Error> {
        // Zip the directory to bytes directly
        utils::io::zip_directory_to_bytes(&self.loc)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
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
    use std::io::Read;
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

    #[test]
    fn test_download_partial() {
        let tempdir = TempDir::new().unwrap();
        let job = Job::new(tempdir.path().to_str().unwrap());

        // Create job directory with some files
        fs::create_dir_all(&job.loc).unwrap();
        fs::write(job.loc.join("file1.txt"), b"data 1").unwrap();
        fs::write(job.loc.join("file2.txt"), b"data 2").unwrap();

        let result = job.download_partial().unwrap();
        assert!(!result.is_empty());

        // Verify the zip contains our files
        let cursor = std::io::Cursor::new(result);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        assert!(archive.len() >= 2);

        let mut file1 = archive.by_name("file1.txt").unwrap();
        let mut contents = String::new();
        file1.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "data 1");
    }

    #[test]
    fn test_download_partial_empty_directory() {
        let tempdir = TempDir::new().unwrap();
        let job = Job::new(tempdir.path().to_str().unwrap());

        // Create empty job directory
        fs::create_dir_all(&job.loc).unwrap();

        let result = job.download_partial().unwrap();
        assert!(!result.is_empty());

        // Verify the zip is empty
        let cursor = std::io::Cursor::new(result);
        let archive = zip::ZipArchive::new(cursor).unwrap();
        assert_eq!(archive.len(), 0);
    }
}
