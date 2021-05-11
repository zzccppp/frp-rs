use log::{error, info};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub bind_ip: String,
    pub server_port: u16,
    pub client: Vec<ClientConfig>,
}
#[derive(Debug, Deserialize, Clone)]
pub struct ClientConfig {
    pub name: String,
    pub port: u16,
    pub protocol: String,
    pub secret_key: String,
}

pub fn init_logger() {
    let env = env_logger::Env::default()
        .filter_or("MY_LOG_LEVEL", "debug")
        .write_style_or("MY_LOG_STYLE", "always");
    env_logger::init_from_env(env);
}

pub fn read_server_configuration() -> Result<Config, ()> {
    if let Ok(s) = std::fs::read_to_string("server.toml") {
        match toml::from_str(s.as_str()) {
            Ok(conf) => {
                info!("Read Config:{:?}", conf);
                return Ok(conf);
            }
            Err(e) => {
                error!("Error while read server.toml");
                error!("{}", e);
                return Err(());
            }
        };
    } else {
        error!("Cannot read config file.");
        return Err(());
    }
}
