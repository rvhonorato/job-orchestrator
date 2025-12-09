use crate::models::status_dto::Status;
use crate::utils;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use utoipa::ToSchema;

#[derive(serde::Serialize, serde::Deserialize, Debug, ToSchema)]
pub struct Payload {
    pub id: u32,
    input: HashMap<String, Vec<u8>>,
    pub status: Status,
    #[schema(value_type = String)]
    pub loc: PathBuf,
}

impl Payload {
    pub fn new() -> Payload {
        Payload {
            id: 0,
            input: HashMap::new(),
            status: Status::Unknown,
            loc: PathBuf::new(),
        }
    }

    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    pub fn add_input(&mut self, filename: String, input: Vec<u8>) {
        self.input.insert(filename, input);
    }

    pub fn set_status(&mut self, status: Status) {
        self.status = status;
    }

    pub fn set_loc(&mut self, loc: PathBuf) {
        self.loc = loc;
    }

    pub fn prepare(&mut self, data_path: &str) -> Result<(), std::io::Error> {
        self.loc = std::path::Path::new(&data_path).join(self.id.to_string());

        // Create directory for this payload
        fs::create_dir_all(&self.loc)?;

        // Dump data to this directory
        self.input.iter_mut().for_each(|(filename, data)| {
            fs::write(self.loc.join(filename), data).expect("Unable to write file")
        });

        Ok(())
    }

    pub fn zip_directory(self) -> Result<Vec<u8>, std::io::Error> {
        // Get everything from the `loc` and return it
        let result = self.loc.join("output.zip");

        // Check if output.zip exists to avoid re-zipping
        if !result.exists() {
            // Not exists, create it by zipping the directory
            utils::io::zip_directory(&self.loc, &result)?
        }

        // Read the output.zip file and return its content
        std::fs::read(&result)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_add_input() {
        let mut p = Payload::new();
        assert_eq!(p.input.len(), 0);
        let data = b"Hello, world!".to_vec();
        let filename = "filename.txt".to_string();
        let expected_map = HashMap::from([(filename.clone(), data.clone())]);
        p.add_input(filename, data.clone());
        assert_eq!(p.input, expected_map);
    }

    #[tokio::test]
    async fn test_prepare() {
        let mut p = Payload::new();
        p.id = 1;
        p.add_input("test.txt".to_string(), b"Test data".to_vec());

        let temp_dir = tempfile::tempdir().unwrap();
        let data_path = temp_dir.path().to_str().unwrap();

        let result = p.prepare(data_path);
        assert!(result.is_ok());

        let expected_path = temp_dir.path().join("1").join("test.txt");
        assert!(expected_path.exists());

        let content = fs::read_to_string(expected_path).unwrap();
        assert_eq!(content, "Test data");
    }
}
