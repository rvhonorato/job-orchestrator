mod config;
mod controllers;
mod datasource;
mod models;
mod routes;
mod services;
mod utils;
use crate::datasource::fs::init_fs;
use crate::routes::router::create_routes;
use crate::{datasource::db::init_db, routes::router::create_client_routes};
use clap::{Parser, Subcommand};
use config::loader::Config;
use services::tasks::{cleaner, getter, runner, sender};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_schedule::{every, Job};

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Run orchestrator server")]
    Server {},

    #[command(about = "Run orchestrator client")]
    Client {},
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize a logger
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .compact()
        .init();

    // Load the configuration
    let config = Config::new().unwrap();

    // Parse command line arguments
    let cli = Cli::parse();

    match &cli.command {
        Commands::Server {} => {
            start_server(config).await?;
        }
        Commands::Client {} => {
            start_client(config).await?;
        }
    }

    Ok(())
}

async fn start_server(config: Config) -> anyhow::Result<()> {
    // Initialize the database
    let pool = init_db(&config.db_path).await;

    // Initialize the filesystem
    let _ = init_fs(&config.data_path).await;

    // Create a scheduled job
    let sender_task = every(500).millisecond().perform(|| {
        let pool_clone = pool.clone();
        let config_clone = config.clone();
        async move { sender(pool_clone, config_clone).await }
    });

    let getter_task = every(500).millisecond().perform(|| {
        let pool_clone = pool.clone();
        let config_clone = config.clone();
        async move { getter(pool_clone, config_clone).await }
    });

    let cleaner_task = every(60).second().perform(|| {
        let pool_clone = pool.clone();
        let config_clone = config.clone();
        async move { cleaner(pool_clone, config_clone).await }
    });

    // Create app
    let app = create_routes(pool.clone(), config.clone());

    // Initialize socket
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;

    tokio::select! {
        _ = sender_task => {},
        _ = getter_task => {},
        _ = cleaner_task => {},
        _ = axum::serve(listener, app.into_make_service()) => {},
    }

    Ok(())
}

async fn start_client(config: Config) -> anyhow::Result<()> {
    // Initialize in-memory database
    let pool = datasource::db::init_payload_db().await;

    // Create a scheduled job
    let runner_task = every(500).millisecond().perform(|| {
        let pool_clone = pool.clone();
        let config_clone = config.clone();
        async move { runner(pool_clone, config_clone).await }
    });

    // Create app
    let client_app = create_client_routes(pool.clone(), config.clone());

    // Initialize socket
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Client listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;

    tokio::select! {
        _ = runner_task => {},
        _ = axum::serve(listener, client_app.into_make_service()) => {},
    };

    Ok(())
}
