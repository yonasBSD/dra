use crate::cli::get_env;
use crate::cli::handlers::{HandlerError, HandlerResult};
use crate::cli::select;
use crate::cli::spinner::Spinner;
use crate::github::client::GithubClient;
use crate::github::error::GithubError;
use crate::github::release::{Asset, Release, Tag};
use crate::github::tagged_asset::TaggedAsset;
use crate::github::{Repository, GITHUB_TOKEN};
use crate::installer::cleanup::InstallCleanup;
use crate::{github, installer};
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct DownloadHandler {
    repository: Repository,
    select: Option<String>,
    tag: Option<Tag>,
    output: Option<PathBuf>,
    install: bool,
}

impl DownloadHandler {
    pub fn new(
        repository: Repository,
        select: Option<String>,
        tag: Option<String>,
        output: Option<PathBuf>,
        install: bool,
    ) -> Self {
        DownloadHandler {
            repository,
            select,
            tag: tag.map(Tag),
            output,
            install,
        }
    }

    pub fn run(&self) -> HandlerResult {
        let client = GithubClient::new(get_env(GITHUB_TOKEN));
        let release = self.fetch_release(&client)?;
        let selected_asset = self.autoselect_or_ask_asset(release)?;
        let output_path = self.choose_output_path(&selected_asset.name);
        Self::download_asset(&client, &selected_asset, &output_path)?;
        self.maybe_install(&output_path)?;
        Ok(())
    }

    fn autoselect_or_ask_asset(&self, release: Release) -> Result<Asset, HandlerError> {
        if let Some(untagged) = self.select.as_ref() {
            Self::autoselect_asset(release, untagged)
        } else {
            Self::ask_select_asset(release.assets)
        }
    }

    fn maybe_install(&self, path: &Path) -> Result<(), HandlerError> {
        if self.install {
            return Self::install_asset(path);
        }
        Ok(())
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
        github::get_release(client, &self.repository, self.tag.as_ref())
            .map_err(Self::release_error)
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
        let spinner = Spinner::download(&selected_asset.name, output_path);
        spinner.start();
        let mut stream =
            github::download_asset(client, selected_asset).map_err(Self::download_error)?;
        let mut destination = Self::create_file(output_path)?;
        std::io::copy(&mut stream, &mut destination).unwrap();
        spinner.stop();
        Ok(())
    }

    fn install_asset(asset_path: &Path) -> Result<(), HandlerError> {
        let spinner = Spinner::install();
        spinner.start();
        installer::install(asset_path)
            .cleanup(asset_path)
            .map_err(|x| HandlerError::new(x.to_string()))?;
        spinner.stop();
        Ok(())
    }

    fn choose_output_path(&self, asset_name: &str) -> PathBuf {
        // If your installing always use current working directory to save the file
        if self.install {
            return PathBuf::from(asset_name);
        }

        self.output
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(asset_name))
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

    fn release_error(e: GithubError) -> HandlerError {
        HandlerError::new(format!("Error fetching the release: {}", e))
    }

    fn download_error(e: GithubError) -> HandlerError {
        HandlerError::new(format!("Error downloading asset: {}", e))
    }
}
