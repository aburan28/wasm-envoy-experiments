mod config;
mod mutation;
mod service;

use std::sync::Arc;

use clap::Parser;
use tokio::sync::RwLock;
use tonic::transport::Server;
use tracing_subscriber::EnvFilter;

use crate::config::ServiceConfig;
use crate::mutation::MutationEngine;
use crate::service::{AdminServiceImpl, ProtoMutationServiceImpl};

pub mod proto {
    tonic::include_proto!("proto_mutation.v1");
}

#[derive(Parser, Debug)]
#[command(name = "ext-proc-service", about = "gRPC proto mutation service")]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.yaml")]
    config: String,

    /// Listen address
    #[arg(short, long)]
    listen: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let cfg = match ServiceConfig::load(&args.config) {
        Ok(c) => {
            tracing::info!(path = %args.config, "loaded configuration");
            c
        }
        Err(e) => {
            tracing::warn!(
                path = %args.config,
                error = %e,
                "failed to load config, using defaults"
            );
            ServiceConfig::default()
        }
    };

    let listen_addr = args
        .listen
        .unwrap_or_else(|| cfg.listen_addr.clone())
        .parse()?;

    let engine = Arc::new(RwLock::new(MutationEngine::from_config(&cfg)));

    let mutation_svc = proto::proto_mutation_service_server::ProtoMutationServiceServer::new(
        ProtoMutationServiceImpl::new(engine.clone()),
    );
    let admin_svc = proto::mutation_admin_service_server::MutationAdminServiceServer::new(
        AdminServiceImpl::new(engine.clone()),
    );

    tracing::info!(%listen_addr, "starting ext-proc-service");

    Server::builder()
        .add_service(mutation_svc)
        .add_service(admin_svc)
        .serve_with_shutdown(listen_addr, shutdown_signal())
        .await?;

    tracing::info!("ext-proc-service stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
    tracing::info!("shutdown signal received");
}
