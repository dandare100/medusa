use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use log::debug;
use thrussh::server::{self, Config as SSHConfig};

use crate::{
	config::{Config as MainConfig, Service},
	protocols::{Error, Protocol},
};

use super::{
	config::{self, Config},
	handler,
};

#[derive(Clone)]
pub struct Server {
	service_name: String,
	service: Arc<Mutex<Service>>,
	config: Arc<Config>,
	ssh_config: Arc<SSHConfig>,
	main_config: Arc<MainConfig>,
}

impl Server {
	pub fn new(
		service_name: String,
		service: Arc<Mutex<Service>>,
		main_config: MainConfig,
	) -> Result<Self, Error> {
		let config = config::from_service(service.lock().as_ref().unwrap());
		let ssh_config = config.to_ssh_config()?;
		let config = Arc::new(config);
		let ssh_config = Arc::new(ssh_config);
		let main_config = Arc::new(main_config);

		Ok(Server {
			service_name,
			service,
			config,
			ssh_config,
			main_config,
		})
	}
}

#[async_trait]
impl Protocol for Server {
	async fn run(&self) {
		debug!("starting ssh on {} ...", &self.config.address);

		thrussh::server::run(self.ssh_config.clone(), &self.config.address, self.clone())
			.await
			.unwrap();
	}
}

impl server::Server for Server {
	type Handler = handler::ClientHandler;

	fn new(&mut self, address: Option<std::net::SocketAddr>) -> handler::ClientHandler {
		handler::ClientHandler::new(
			self.service_name.clone(),
			self.service.clone(),
			address.unwrap(),
			self.config.clone(),
			self.main_config.clone(),
		)
	}
}
