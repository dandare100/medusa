use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};

use clap::{AppSettings, Clap};
use futures::future;
use glob::glob;
use log::{debug, error, info};

#[derive(Clap)]
#[clap(author, about, version)]
#[clap(setting = AppSettings::ColoredHelp)]
struct Options {
    /// Path containing service YAML files.
    #[clap(short, long, default_value = "services.d")]
    pub services: String,
    /// Record files destination path.
    #[clap(short, long, default_value = "records")]
    pub records: String,
    /// Enable debug verbosity.
    #[clap(long)]
    pub debug: bool,
}

mod command;
mod config;
mod protocols;
mod record;

#[tokio::main]
async fn main() {
    let mut options: Options = Options::parse();

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(if options.debug { "debug" } else { "info" }),
    )
    .format_module_path(false)
    .format_target(false)
    .init();

    debug!("creating {}", &options.records);
    fs::create_dir_all(&options.records).expect("could not create record path");
    debug!("creating {}", &options.services);
    fs::create_dir_all(&options.services).expect("could not create services path");

    options.records = fs::canonicalize(&options.records)
        .expect("could not canonicalize records path")
        .to_str()
        .unwrap()
        .to_owned();

    options.services = fs::canonicalize(&options.services)
        .expect("could not canonicalize services path")
        .to_str()
        .unwrap()
        .to_owned();

    debug!("loading services from {} ...", &options.services);
    let mut config = config::Config::new();

    config.records.path = options.records;
    for entry in glob(&format!("{}/**/*.yml", options.services)).unwrap() {
        match entry {
            Ok(path) => {
                debug!("loading {}", path.display());

                let service_name = path
                    .to_str()
                    .unwrap()
                    .replace(&options.services, "")
                    .trim_start_matches('/')
                    .trim_end_matches(".yml")
                    .replace("/", "-")
                    .to_string();

                let data = fs::read_to_string(&path).expect("could not read service file");
                let service: config::Service =
                    serde_yaml::from_str(&data).expect("error parsing service file");

                config.services.insert(service_name, service);
            }
            Err(e) => error!("{:?}", e),
        }
    }

    let mut services = HashMap::new();
    let mut futures = Vec::new();

    for (name, service_config) in &config.services {
        debug!("building service {} handler", name);
        match protocols::factory(
            &service_config.proto,
            name,
            Arc::new(Mutex::new(service_config.clone())),
            config.clone(),
        ) {
            Ok(service) => {
                services.insert(name, (service, service_config));
            }
            Err(e) => {
                error!("{}", e);
                return;
            }
        }
    }

    for (name, (service, service_config)) in &services {
        info!(
            "starting {} on {} ({}) ...",
            name, service_config.address, service_config.proto
        );
        futures.push(service.run());
    }

    if futures.len() > 1 {
        info!("all services started");
    } else {
        info!("service started");
    }

    future::join_all(futures).await;
}
