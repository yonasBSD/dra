use crate::installer::error::{InstallError, InstallErrorMapErr};
use std::process::{Command, Output};

pub fn exec_command(name: &str, command: &mut Command) -> Result<(), InstallError> {
    command
        .output()
        .map_fatal_err(format!("An error occurred executing '{}'", name))
        .and_then(|output| handle_command_output(name, output))
}

fn handle_command_output(name: &str, output: Output) -> Result<(), InstallError> {
    if output.status.success() {
        Ok(())
    } else {
        Err(InstallError::Fatal(format!(
            "An error occurred while executing (status: {}):\n  {}",
            output
                .status
                .code()
                .map(|x| x.to_string())
                .unwrap_or_else(|| "NA".into()),
            String::from_utf8(output.stderr).unwrap_or_else(|_| format!("Unknown {} error", name))
        )))
    }
}
