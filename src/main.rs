use anyhow::Result;
use clap::Parser;
use glance::server::rpc::RpcServer;

#[derive(Parser)]
#[command(name = "glance", version, about = "Large file viewer daemon for IDEs — communicates over stdio")]
struct Cli {}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let _cli = Cli::parse();

    tracing::info!("glance daemon started");

    let server = RpcServer::new();
    server.run().await
}
