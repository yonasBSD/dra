use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::installer::InstallerResult;

use super::{
    error::{InstallError, InstallErrorMapErr},
    file::SupportedFileInfo,
    Executable,
};

pub struct CompressedFileInstaller;

impl CompressedFileInstaller {
    pub fn gz(
        file_info: SupportedFileInfo,
        destination_dir: &Path,
        executable: &Executable,
    ) -> InstallerResult {
        Self::decompress_and_move(
            |file| Box::new(flate2::read::GzDecoder::new(file)),
            file_info,
            destination_dir,
            executable,
        )
    }

    pub fn xz(
        file_info: SupportedFileInfo,
        destination_dir: &Path,
        executable: &Executable,
    ) -> InstallerResult {
        Self::decompress_and_move(
            |file| Box::new(xz2::read::XzDecoder::new(file)),
            file_info,
            destination_dir,
            executable,
        )
    }

    pub fn bz2(
        file_info: SupportedFileInfo,
        destination_dir: &Path,
        executable: &Executable,
    ) -> InstallerResult {
        Self::decompress_and_move(
            |file| Box::new(bzip2::read::BzDecoder::new(file)),
            file_info,
            destination_dir,
            executable,
        )
    }

    fn decompress_and_move<D>(
        decode: D,
        file_info: SupportedFileInfo,
        destination_dir: &Path,
        executable: &Executable,
    ) -> Result<(), InstallError>
    where
        D: FnOnce(File) -> Box<dyn Read>,
    {
        let compressed_file = File::open(&file_info.path)
            .map_fatal_err(format!("Error opening {}", file_info.path.display()))?;

        let mut stream = decode(compressed_file);

        let executable_path = destination_dir.join(executable.name());
        let mut destination_file = File::create(&executable_path)
            .map_fatal_err(format!("Error creating {}", executable_path.display()))?;

        std::io::copy(&mut stream, &mut destination_file)
            .map_fatal_err(format!("Error saving {}", executable_path.display()))?;

        set_executable_permissions(&executable_path)?;

        Ok(())
    }
}

#[cfg(target_family = "unix")]
fn set_executable_permissions(path: &Path) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, PermissionsExt::from_mode(0o755)).map_fatal_err(format!(
        "Cannot set executable permissions on {}",
        path.display(),
    ))
}

#[cfg(target_os = "windows")]
fn set_executable_permissions(path: &Path) -> Result<(), InstallError> {
    Ok(())
}
