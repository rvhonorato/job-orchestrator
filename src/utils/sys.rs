use std::{
    path::Path,
    process::{Child, Command},
};

#[derive(Debug)]
pub struct ManagedProcess {
    child: Child,
}

impl ManagedProcess {
    pub fn new(loc: &Path) -> std::io::Result<Self> {
        let run_script = loc.join("run.sh");
        let child = Command::new("bash")
            .arg(run_script)
            .current_dir(loc)
            .spawn()?;
        Ok(Self { child })
    }

    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }

    pub fn get_exit_status(&mut self) -> Option<i32> {
        self.child
            .try_wait()
            .ok()
            .flatten()
            .map(|status| status.code())?
    }

    pub fn is_running(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }
}
