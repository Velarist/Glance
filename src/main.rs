use anyhow::Result;
use clap::{Parser, Subcommand};
use glance::cli::output::Format;
use glance::server::rpc::RpcServer;

#[derive(Parser)]
#[command(name = "glance", version, about = "Large file viewer — daemon + developer CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the JSON-RPC daemon over stdio (default when no subcommand given)
    Serve,

    /// Show file metadata: line count, size, format
    Info {
        path: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Read lines from a file
    Read {
        path: String,
        #[arg(long, default_value_t = 0)]
        offset: u64,
        #[arg(long, default_value_t = 20)]
        limit: u64,
        /// Pretty-print JSON content (JSONL files only)
        #[arg(long)]
        pretty: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Search for a query string in a file
    Search {
        path: String,
        query: String,
        /// Treat query as a regex pattern
        #[arg(long)]
        regex: bool,
        /// Maximum number of results to show
        #[arg(long, default_value_t = 50)]
        max: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Count lines matching a query
    Count {
        path: String,
        query: String,
        /// Treat query as a regex pattern
        #[arg(long)]
        regex: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Validate a JSONL file — report lines that are not valid JSON
    Validate {
        path: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Command::Serve) => {
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_env_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive(tracing::Level::INFO.into()),
                )
                .init();
            tracing::info!("glance daemon started");
            let server = RpcServer::new();
            server.run().await
        }

        Some(Command::Info { path, json }) =>
            glance::cli::info::run(&path, Format::from_flag(json)),

        Some(Command::Read { path, offset, limit, pretty, json }) =>
            glance::cli::read::run(&path, offset, limit, pretty, Format::from_flag(json)),

        Some(Command::Search { path, query, regex, max, json }) =>
            glance::cli::search::run(&path, &query, regex, max, Format::from_flag(json)),

        Some(Command::Count { path, query, regex, json }) =>
            glance::cli::count::run(&path, &query, regex, Format::from_flag(json)),

        Some(Command::Validate { path, json }) =>
            glance::cli::validate::run(&path, Format::from_flag(json)),
    }
}
