use crate::models::status_dto::Status;
use crate::services::client::ClientError;
use crate::utils;
use crate::utils::sys::is_pid_running;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use utoipa::ToSchema;

#[derive(serde::Serialize, serde::Deserialize, Debug, ToSchema)]
pub struct Payload {
    pub id: u32,
    input: HashMap<String, Vec<u8>>,
    pub status: Status,
    #[schema(value_type = String)]
    pub loc: PathBuf,
    pub pid: u32,
    pub killed: bool,
}

const RUN_FILE: &str = "run.sh";
const OUTPUT_FILE: &str = "output.zip";
const EXIT_FILE: &str = ".orchestrator.exit";

impl Payload {
    pub fn new() -> Payload {
        Payload {
            id: 0,
            input: HashMap::new(),
            status: Status::Unknown,
            loc: PathBuf::new(),
            pid: 0,
            killed: false,
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
        let result = self.loc.join(OUTPUT_FILE);

        // Check if output.zip exists to avoid re-zipping
        if !result.exists() {
            // Not exists, create it by zipping the directory
            utils::io::zip_directory(&self.loc, &result)?
        }

        // Read the output.zip file and return its content
        std::fs::read(&result)
    }

    pub fn execute(&mut self) -> Result<(), ClientError> {
        let run_script = self.loc.join(RUN_FILE);
        utils::io::validate_script(&run_script)?;

        let child = Command::new("bash")
            .arg(run_script)
            .current_dir(&self.loc)
            .spawn()
            .map_err(|_| ClientError::Execution)?;

        self.pid = child.id();

        Ok(())
    }

    pub fn kill(&mut self) -> std::io::Result<()> {
        if self.pid == 0 {
            return Ok(());
        }
        std::process::Command::new("kill")
            .arg("-TERM")
            .arg(self.pid.to_string())
            .status()?;
        Ok(())
    }

    pub fn is_exit(&mut self) -> bool {
        self.loc.join(EXIT_FILE).exists()
    }

    pub fn is_killed(&self) -> bool {
        self.killed
    }

    pub fn is_running(&self) -> Option<bool> {
        if self.pid != 0 {
            Some(is_pid_running(self.pid))
        } else {
            None
        }
    }

    pub fn status_code(&mut self) -> Option<i32> {
        // NOTE: Since the process is spawned, the system will discard the exit status
        // so the only way we can reliable capture it back is by using
        // `trap 'echo "$?" > .orchestrator.exit' EXIT` in the top of the `run.sh`
        // script
        let exit_file = self.loc.join(EXIT_FILE);
        if exit_file.exists()
            && let Ok(content) = std::fs::read_to_string(&exit_file)
        {
            content.trim().parse::<i32>().ok()
        } else {
            None
        }
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
