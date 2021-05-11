use log::error;
use serde::{Deserialize, Serialize};
use tokio::net::TcpSocket;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientRegisterMessage {
    pub name: String,
    pub secret: String,
    pub protocol: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum RegisterResponse {
    Succ{
        uuid: String
    },
    Failed{
        reason: String
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectionState {
    pub last_heart_beat: u32, //TODO
    pub register_time: u128,
    pub name: String,
}
