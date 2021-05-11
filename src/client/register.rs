use std::net::SocketAddr;

use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpSocket,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientRegisterMessage {
    pub name: String,
    pub secret: String,
    pub protocol: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum RegisterResponse {
    Succ { uuid: String },
    Failed { reason: String },
}

pub async fn register(
    register_address: SocketAddr,
    name: &String,
    secret: &String,
    protocol: &String,
) -> Result<RegisterResponse, ()> {
    let mut stream = if let Ok(socket) = TcpSocket::new_v4() {
        let stream = socket.connect(register_address).await;
        if let Err(e) = stream {
            error!(
                "Cannot connect to {} for register",
                register_address.to_string()
            );
            error!("{}", e.to_string());
            return Err(());
        }
        stream.unwrap()
    } else {
        error!("Cannot create TcpSocket");
        return Err(());
    };
    info!("Connect to {} for register.", register_address.to_string());
    let msg = ClientRegisterMessage {
        name: name.clone(),
        secret: secret.clone(),
        protocol: protocol.clone(),
    };
    let msg = bincode::serialize(&msg).unwrap();
    stream.write_all(msg.as_slice()).await.unwrap();

    let mut recv_buf = [0u8; 512];
    let size = stream.read(&mut recv_buf).await.unwrap();
    let resp: RegisterResponse = bincode::deserialize(&recv_buf[0..size]).unwrap();
    info!("Response: {:?}", resp);
    Ok(resp)
}
