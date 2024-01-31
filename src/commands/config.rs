//! Config CLI command of pulsar is about setting either or all of the
//! parameters:
//! - chain
//! - farm size
//! - reward address
//! - node directory
//! - farm directory
//!
//! and showing the set config details.
//!
//! ## Usage
//!
//! ### Show
//! ```
//! $ pulsar config -s
//! Current Config set as:
//! {
//!   "chain": "Gemini3g",
//!   "farmer": {
//!     "reward_address": "06fef8efdd808a95500e5baee2bcde4cf8d5e1191b2b3f93065f10f0e4648c09",
//!     "farm_directory": "/Users/abhi3700/Library/Application Support/pulsar/farms",
//!     "farm_size": "3.0 GB"
//!   },
//!   "node": {
//!     "directory": "/Users/abhi3700/Library/Application Support/pulsar/node",
//!     "name": "abhi3700"
//!   }
//! }
//! in file: "/Users/abhi3700/Library/Application Support/pulsar/settings.toml"
//! ```
//!
//! ### Chain
//! ```
//! $ pulsar config -c devnet
//! ```
//!
//! ### Farm size
//! ```
//! $ pulsar config -f 3GB
//! ```
//!
//! ### Reward address
//!
//! ```
//! $ pulsar config -r 5CDstQSbxPrPAaRTuVR2n9PHuhGYnnQvXdbJSQmodD5i16x2
//! ```
//!
//! ### Node directory
//! TODO: add usage
//!
//! ### Farm directory
//! TODO: add usage

use std::fs;

use color_eyre::eyre::{self, bail};

use crate::commands::wipe::wipe;
use crate::config::{parse_config, parse_config_path, Config};
use crate::utils::{
    chain_parser, create_and_move_data, dir_parser, reward_address_parser, size_parser,
};

// function for config cli command
pub(crate) async fn config(
    show: bool,
    chain: Option<String>,
    farm_size: Option<String>,
    reward_address: Option<String>,
    node_dir: Option<String>,
    farm_dir: Option<String>,
) -> eyre::Result<()> {
    // Define the path to your settings.toml file
    let config_path = parse_config_path()?;

    // if config file doesn't exist, then throw error
    if !config_path.exists() {
        bail!("Config file: \"settings.toml\" not found.\nPlease use `pulsar init` command first.");
    }

    // Load the current configuration
    let mut config: Config = parse_config()?;

    if show {
        // Display the current configuration as JSON
        // Serialize `config` to a pretty-printed JSON string
        let serialized = serde_json::to_string_pretty(&config)?;
        println!(
            "Current Config set as: \n{}\nin file: {:?}",
            serialized,
            config_path.to_str().expect("Expected stringified config path")
        );
    } else {
        // no options provided
        if chain.is_none()
            && farm_size.is_none()
            && reward_address.is_none()
            && node_dir.is_none()
            && farm_dir.is_none()
        {
            println!("At least one option has to be provided.\nTry `pulsar config -h`");
            return Ok(());
        }

        // update (optional) the chain
        if let Some(c) = chain {
            config.chain = chain_parser(&c)?;
            println!("Chain updated as {:?}", c);

            // wipe everything (farmer, node, summary) except config file
            wipe(true, true, true, false).await.expect("Error while wiping.");
        }

        // update (optional) the farm size
        if let Some(ref f) = farm_size {
            let farm_size = size_parser(&f)?;
            config.farmer.farm_size = farm_size;
        }

        // update (optional) the reward address
        if let Some(ref r) = reward_address {
            let reward_address = reward_address_parser(&r)?;
            config.farmer.reward_address = reward_address;
        }

        // update (optional) the node directory
        if let Some(ref n) = node_dir {
            let node_dir = dir_parser(&n).expect("Invalid node directory");
            create_and_move_data(config.node.directory.clone(), node_dir.clone())
                .expect("Error in setting new node directory.");
            config.node.directory = node_dir;
        }

        // update (optional) the farm directory
        if let Some(ref fd) = farm_dir {
            let farm_dir = dir_parser(&fd).expect("Invalid farm directory");
            create_and_move_data(config.farmer.farm_directory.clone(), farm_dir.clone())
                .expect("Error in setting new farm directory.");
            if farm_dir.exists() {
                config.farmer.farm_directory = farm_dir;
            }
        }

        // Save the updated configuration back to the file
        // FIXME: might be needed to use `to_string_pretty`
        fs::write(config_path, toml::to_string(&config)?)?;
    }

    Ok(())
}
