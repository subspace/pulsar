use color_eyre::eyre::Result;
use subspace_sdk::{Node, PlotDescription};

use crate::config::parse_config;
use crate::utils::node_directory_getter;

pub(crate) async fn wipe() -> Result<()> {
    let config_args = match parse_config() {
        Ok(args) => args,
        Err(_) => {
            println!("could not read your config. You must have a valid config in order to wipe. Aborting...");
            return Ok(());
        }
    };
    let node_directory = node_directory_getter();
    Node::wipe(node_directory).await?;
    println!("Node is wiped!");

    // TODO: modify here when supporting multi-plot
    let plot = PlotDescription {
        directory: config_args.farmer_config_args.plot.directory,
        space_pledged: config_args.farmer_config_args.plot.space_pledged,
    };
    println!("Farmer is wiped!");

    plot.wipe().await?;

    Ok(())
}