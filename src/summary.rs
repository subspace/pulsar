/// Stores the summary of the farming process into a file.
/// This allows to retrieve farming information with `info` command,
/// and also store the amount of potentially farmed blocks during the initial
/// plotting progress, so that progress bar won't be affected with `println!`,
/// and user will still know about them when initial plotting is finished.
use std::fs::remove_file;
use std::path::PathBuf;
use std::sync::Arc;

use color_eyre::eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use subspace_sdk::ByteSize;
use tokio::fs::{create_dir_all, read_to_string, File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::instrument;

// TODO: delete this when https://github.com/toml-rs/toml/issues/540 is solved
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct Rewards(pub(crate) u128);

impl TryFrom<String> for Rewards {
    type Error = <u128 as std::str::FromStr>::Err;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse().map(Self)
    }
}

impl From<Rewards> for String {
    fn from(Rewards(r): Rewards) -> Self {
        r.to_string()
    }
}

impl std::fmt::Display for Rewards {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// struct for flexibly updating the fields of the summary
#[derive(Default, Debug)]
pub(crate) struct SummaryUpdateFields {
    pub(crate) is_plotting_finished: bool,
    pub(crate) is_new_block_farmed: bool,
    pub(crate) is_new_vote: bool,
    pub(crate) maybe_new_reward: Option<Rewards>,
    pub(crate) maybe_last_block: Option<u64>,
}

/// Struct for holding the info of what to be displayed with the `info` command,
/// and printing rewards to user in `farm` command
#[derive(Clone, Debug)]
pub(crate) struct Summary {
    pub(crate) inner: Arc<Mutex<SummaryInner>>,
}

/// inner struct for the `Summary`
#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub(crate) struct SummaryInner {
    pub(crate) initial_plotting_finished: bool,
    pub(crate) farmed_block_count: u64,
    pub(crate) vote_count: u64,
    pub(crate) total_rewards: Rewards,
    pub(crate) user_space_pledged: ByteSize,
    pub(crate) last_block_num: u64,
}

impl Summary {
    #[instrument]
    pub(crate) async fn new(summary_file: SummaryFile) -> Result<Summary> {
        let inner = summary_file.parse_summary_file().await?;
        Ok(Summary { inner: Arc::new(Mutex::new(inner)) })
    }

    /// updates the summary file, and returns the content of the new summary
    ///
    /// this function will be called by the farmer when
    /// the status of the `plotting_finished`
    /// or value of `farmed_block_count` changes
    #[instrument]
    pub(crate) async fn update(
        &mut self,
        SummaryUpdateFields {
            is_plotting_finished,
            is_new_block_farmed,
            is_new_vote,
            maybe_new_reward,
            maybe_last_block,
        }: SummaryUpdateFields,
    ) -> Result<SummaryInner> {
        let mut guard = self.inner.lock().await;
        if is_plotting_finished {
            guard.initial_plotting_finished = true;
        }
        if is_new_block_farmed {
            guard.farmed_block_count += 1;
        }
        if is_new_vote {
            guard.vote_count += 1;
        }
        if let Some(new_reward) = maybe_new_reward {
            guard.total_rewards = new_reward;
        }
        if let Some(last_block) = maybe_last_block {
            guard.last_block_num = last_block;

            // write to persistent storage at every 100 blocks
            if last_block % 100 == 0 {
                let new_summary =
                    toml::to_string(&*guard).context("Failed to serialize Summary")?;

                // this will only create the pointer, and will not override the file
                let summary_file = SummaryFile::new(None).await?;
                let guard = summary_file.inner.lock().await;
                let mut buffer =
                    OpenOptions::new().write(true).truncate(true).open(&*guard).await?;
                buffer.write_all(new_summary.as_bytes()).await?;
                buffer.flush().await?;
            }
        }

        Ok(*guard)
    }

    /// parses the summary struct and returns [`SummaryInner`]
    #[instrument]
    pub(crate) async fn parse_summary(&self) -> Result<SummaryInner> {
        let guard = self.inner.lock().await;

        Ok(*guard)
    }
}

/// utilizing persistent storage for the information to be displayed for the
/// `info` command
#[derive(Debug, Clone)]
pub(crate) struct SummaryFile {
    inner: Arc<Mutex<PathBuf>>,
}

impl SummaryFile {
    /// creates new summary file pointer
    ///
    /// if user_space_pledged is provided, it creates a new summary file
    /// else, it tries to read the existing summary file
    #[instrument]
    pub(crate) async fn new(user_space_pledged: Option<ByteSize>) -> Result<SummaryFile> {
        let summary_path = summary_path();
        let summary_dir = dirs::data_local_dir()
            .expect("couldn't get the default local data directory!")
            .join("subspace-cli");

        // providing `Some` value for `user_space_pledged` means, we are creating a new
        // file, so, first check if the file exists to not erase its content
        if let Some(user_space_pledged) = user_space_pledged {
            // File::create will truncate the existing file, so first
            // check if the file exists, if not, `open` will return an error
            // in this case, create the file and necessary directories
            // if file exists, we do nothing
            if File::open(&summary_path).await.is_err() {
                let _ = create_dir_all(&summary_dir).await;
                let _ = File::create(&summary_path).await;
                let initialization = SummaryInner {
                    initial_plotting_finished: false,
                    farmed_block_count: 0,
                    vote_count: 0,
                    total_rewards: Rewards(0),
                    user_space_pledged,
                    last_block_num: 0,
                };
                let summary_text =
                    toml::to_string(&initialization).context("Failed to serialize Summary")?;
                OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&summary_path)
                    .await?
                    .write_all(summary_text.as_bytes())
                    .await?;
            }
        }

        Ok(SummaryFile { inner: Arc::new(Mutex::new(summary_path)) })
    }

    /// parses the summary file and returns [`SummaryInner`]
    #[instrument]
    pub(crate) async fn parse_summary_file(&self) -> Result<SummaryInner> {
        let guard = self.inner.lock().await;
        let inner: SummaryInner = toml::from_str(&read_to_string(&*guard).await?)?;

        Ok(inner)
    }
}

/// deletes the summary file
#[instrument]
pub(crate) fn delete_summary() -> Result<()> {
    remove_file(summary_path()).context("couldn't delete summary file")
}

/// returns the path for the summary file
#[instrument]
fn summary_path() -> PathBuf {
    let summary_path =
        dirs::data_local_dir().expect("couldn't get the default local data directory!");
    summary_path.join("subspace-cli").join("summary.toml")
}
