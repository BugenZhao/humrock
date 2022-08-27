use std::path::Path;

use anyhow::Result;
use humrock::{humrock::Humrock, server::HumrockService};
use proto::state_store_server::StateStoreServer;

fn main() {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(run())
        .unwrap();
}

async fn run() -> Result<()> {
    let humrock = Humrock::new(Path::new("default"));

    tonic::transport::Server::builder()
        .add_service(StateStoreServer::new(HumrockService(humrock)))
        .serve("0.0.0.0:1919".parse()?)
        .await?;

    Ok(())
}
