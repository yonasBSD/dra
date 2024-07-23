use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::cli::get_env;
use crate::cli::handlers::common::fetch_release_for;
use crate::cli::handlers::download::find_asset_by_system::find_asset_by_system;
use crate::cli::handlers::{HandlerError, HandlerResult};
use crate::cli::progress_bar::ProgressBar;
use crate::cli::select;
use crate::cli::spinner::Spinner;
use crate::github::client::GithubClient;
use crate::github::error::GithubError;
use crate::github::release::{Asset, Release, Tag};
use crate::github::tagged_asset::TaggedAsset;
use crate::github::{Repository, GITHUB_TOKEN};
use crate::installer::cleanup::InstallCleanup;
use crate::{github, installer};

mod find_asset_by_system;

pub struct DownloadHandler {
    repository: Repository,
    mode: DownloadMode,
    tag: Option<Tag>,
    output: Option<PathBuf>,
    install: bool,
    install_new: Install,
}

enum DownloadMode {
    Interactive,
    Selection(String),
    Automatic,
}

impl DownloadMode {
    fn new(select: Option<String>, automatic: bool) -> Self {
        match (select, automatic) {
            (Some(x), _) => Self::Selection(x),
            (_, true) => Self::Automatic,
            (None, false) => Self::Interactive,
        }
    }
}

enum Install {
    No,
    Yes(String),
}

impl Install {
    fn new(install: Option<Option<String>>, repository: &Repository) -> Self {
        match install {
            Some(executable_name) => {
                Self::Yes(executable_name.unwrap_or_else(|| repository.repo.clone()))
            }
            _ => Self::No,
        }
    }

    fn as_bool(&self) -> bool {
        match self {
            Self::No => false,
            Self::Yes(_) => true,
        }
    }
}

impl DownloadHandler {
    pub fn new(
        repository: Repository,
        select: Option<String>,
        automatic: bool,
        tag: Option<String>,
        output: Option<PathBuf>,
        install: Option<Option<String>>,
    ) -> Self {
        let install_new = Install::new(install, &repository);
        DownloadHandler {
            repository,
            mode: DownloadMode::new(select.clone(), automatic),
            tag: tag.map(Tag),
            output,
            install: install_new.as_bool(),
            install_new,
        }
    }

    pub fn run(&self) -> HandlerResult {
        let client = GithubClient::new(get_env(GITHUB_TOKEN));
        let release = self.fetch_release(&client)?;
        let selected_asset = self.select_asset(release)?;
        let output_path = self.choose_output_path(&selected_asset.name);
        Self::download_asset(&client, &selected_asset, &output_path)?;
        self.maybe_install(&selected_asset.name, &output_path)?;
        Ok(())
    }

    fn select_asset(&self, release: Release) -> Result<Asset, HandlerError> {
        match &self.mode {
            DownloadMode::Interactive => Self::ask_select_asset(release.assets),
            DownloadMode::Selection(untagged) => Self::autoselect_asset(release, untagged),
            DownloadMode::Automatic => {
                let os = std::env::consts::OS;
                let arch = std::env::consts::ARCH;
                find_asset_by_system(os, arch, release.assets).ok_or_else(|| {
                    Self::automatic_download_error(&self.repository, &release.tag, os, arch)
                })
            }
        }
    }

    fn automatic_download_error(
        repository: &Repository,
        release: &Tag,
        os: &str,
        arch: &str,
    ) -> HandlerError {
        let title = urlencoding::encode("Error: automatic download of asset");
        let body = format!(
            "## dra version\n{}\n## Bug report\nRepository: {}\nRelease: {}\nOS: {}\nARCH: {}",
            env!("CARGO_PKG_VERSION"),
            repository,
            release.0,
            os,
            arch
        );
        let body = urlencoding::encode(&body);
        let issue_url = format!(
            "https://github.com/devmatteini/dra/issues/new?title={}&body={}",
            title, body
        );
        HandlerError::new(format!(
            "Cannot find asset that matches your system {} {}\nIf you think this is a bug, please report the issue: {}",
            os, arch, issue_url
        ))
    }

    fn maybe_install(&self, asset_name: &str, path: &Path) -> Result<(), HandlerError> {
        match &self.install_new {
            Install::No => Ok(()),
            Install::Yes(executable_name) => {
                let destination_dir = self.output_dir_or_cwd()?;
                let spinner = Spinner::install_layout();
                spinner.show();

                installer::install(
                    asset_name.to_string(),
                    path,
                    &destination_dir,
                    &executable_name,
                )
                .cleanup(path)
                .map_err(|x| HandlerError::new(x.to_string()))?;

                spinner.finish();
                Ok(())
            }
        }
    }

    fn output_dir_or_cwd(&self) -> Result<PathBuf, HandlerError> {
        self.output
            .as_ref()
            .map(|x| Self::dir_or_error(x))
            .unwrap_or_else(|| {
                std::env::current_dir().map_err(|x| {
                    HandlerError::new(format!("Error retrieving current directory: {}", x))
                })
            })
    }

    fn autoselect_asset(release: Release, untagged: &str) -> Result<Asset, HandlerError> {
        let asset_name = TaggedAsset::tag(&release.tag, untagged);
        release
            .assets
            .into_iter()
            .find(|x| x.name == asset_name)
            .ok_or_else(|| HandlerError::new(format!("No asset found for {}", untagged)))
    }

    fn fetch_release(&self, client: &GithubClient) -> Result<Release, HandlerError> {
        fetch_release_for(client, &self.repository, self.tag.as_ref())
    }

    fn ask_select_asset(assets: Vec<Asset>) -> select::AskSelectAssetResult {
        select::ask_select_asset(
            assets,
            select::Messages {
                select_prompt: "Pick the asset to download",
                quit_select: "No asset selected",
            },
        )
    }

    fn download_asset(
        client: &GithubClient,
        selected_asset: &Asset,
        output_path: &Path,
    ) -> Result<(), HandlerError> {
        let progress_bar = ProgressBar::download_layout(&selected_asset.name, output_path);
        progress_bar.show();
        let (mut stream, maybe_content_length) =
            github::download_asset_stream(client, selected_asset).map_err(Self::download_error)?;
        progress_bar.set_length(maybe_content_length);

        let mut destination = Self::create_file(output_path)?;
        let mut total_bytes = 0;
        let mut buffer = [0; 1024];
        while let Ok(bytes) = stream.read(&mut buffer) {
            if bytes == 0 {
                break;
            }

            destination
                .write(&buffer[..bytes])
                .map_err(|x| Self::write_err(&selected_asset.name, output_path, x))?;

            total_bytes += bytes as u64;
            progress_bar.update_progress(total_bytes);
        }
        progress_bar.finish();
        Ok(())
    }

    pub fn choose_output_path(&self, asset_name: &str) -> PathBuf {
        choose_output_path_from(self.output.as_ref(), self.install, asset_name, Path::is_dir)
    }

    fn create_file(path: &Path) -> Result<File, HandlerError> {
        File::create(path).map_err(|e| {
            HandlerError::new(format!(
                "Failed to create the file {}: {}",
                path.display(),
                e
            ))
        })
    }

    pub fn write_err(asset_name: &str, output_path: &Path, error: std::io::Error) -> HandlerError {
        HandlerError::new(format!(
            "Error saving {} to {}: {}",
            asset_name,
            output_path.display(),
            error
        ))
    }

    fn dir_or_error(path: &Path) -> Result<PathBuf, HandlerError> {
        if path.is_dir() {
            Ok(PathBuf::from(path))
        } else {
            Err(HandlerError::new(format!(
                "{} is not a directory",
                path.display()
            )))
        }
    }

    fn download_error(e: GithubError) -> HandlerError {
        HandlerError::new(format!("Error downloading asset: {}", e))
    }
}

fn choose_output_path_from<IsDir>(
    output: Option<&PathBuf>,
    install: bool,
    asset_name: &str,
    is_dir: IsDir,
) -> PathBuf
where
    IsDir: FnOnce(&Path) -> bool,
{
    if install {
        return crate::cli::temp_file::temp_file();
    }

    output
        .map(|path| {
            if is_dir(path) {
                path.join(asset_name)
            } else {
                path.to_path_buf()
            }
        })
        .unwrap_or_else(|| PathBuf::from(asset_name))
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    const INSTALL: bool = true;
    const NO_INSTALL: bool = false;
    const ANY_ASSET_NAME: &str = "ANY_ASSET_NAME";

    /// CLI command:
    /// dra download -i -o /some/path <REPO> or dra download -i <REPO>
    #[test_case(Some(PathBuf::from("/some/path")); "any_custom_output")]
    #[test_case(None; "no_output")]
    fn choose_output_install(output: Option<PathBuf>) {
        let result = choose_output_path_from(output.as_ref(), INSTALL, ANY_ASSET_NAME, not_dir);

        assert!(result
            .to_str()
            .expect("Error: no path available")
            .contains("dra-"))
    }

    /// CLI command:
    /// dra download -s my_asset.deb <REPO>
    /// output: $PWD/my_asset.deb
    #[test]
    fn choose_output_nothing_chosen() {
        let result = choose_output_path_from(None, NO_INSTALL, "my_asset.deb", not_dir);

        assert_eq!(PathBuf::from("my_asset.deb"), result)
    }

    /// CLI command:
    /// dra download -o /some/path.zip <REPO>
    /// output: /some/path.zip
    #[test]
    fn choose_output_custom_file_path() {
        let output = PathBuf::from("/some/path.zip");

        let result = choose_output_path_from(Some(&output), NO_INSTALL, ANY_ASSET_NAME, not_dir);

        assert_eq!(output, result)
    }

    /// CLI command:
    /// dra download -s my_asset.tar.gz -o /my/custom-dir/ <REPO>
    /// output: /my/custom-dir/my_asset.tar.gz
    #[test]
    fn choose_output_custom_directory_path() {
        let output = PathBuf::from("/my/custom-dir/");
        let asset_name = "my_asset.tar.gz";

        let result = choose_output_path_from(Some(&output), NO_INSTALL, asset_name, is_dir);

        let expected = output.join(asset_name);
        assert_eq!(expected, result);
    }

    fn is_dir(_: &Path) -> bool {
        true
    }

    fn not_dir(_: &Path) -> bool {
        false
    }
}
