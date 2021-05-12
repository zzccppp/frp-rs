use std::{
    net::{SocketAddr, TcpListener},
    sync::Arc,
};

use crate::register::RegisterResponse;
use initialization::{init_logger, read_configuration};
use log::{debug, error, info};
use register::register;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpSocket, TcpStream},
    sync::Mutex,
};

pub mod initialization;
pub mod register;

#[tokio::main]
pub async fn main() -> Result<(), ()> {
    init_logger();
    let conf = read_configuration().unwrap();

    let mut handles = Vec::new();

    for client in &conf.client {
        let client = client.clone();
        let target_addr_str =
            conf.server_ip.clone() + ":" + client.remote_port.to_string().as_str();
        let addr: SocketAddr = target_addr_str.parse().unwrap();
        let main_addr_str = conf.server_ip.clone() + ":" + conf.server_port.to_string().as_str();
        let local_addr_str = client.local_ip.clone() + ":" + client.local_port.to_string().as_str();
        let local_addr: SocketAddr = local_addr_str.parse().unwrap();
        let main_addr = main_addr_str.parse().unwrap();
        let handle = tokio::spawn(async move {
            if let Ok(resp) = register(
                main_addr,
                &client.name,
                &client.secret_key,
                &"tcp".to_string(),
            )
            .await
            {
                match resp {
                    RegisterResponse::Succ { uuid } => {
                        let uuid_bin = bincode::serialize(uuid).unwrap();
                        let conn = TcpSocket::new_v4().unwrap().connect(target_addr_str).await;
                        if let Ok(stream) = conn {
                            stream.write(uuid_bin.as_slice()).await.unwrap();
                            process(stream);
                        } else {
                            error!("Failed to connect target address:{}", target_addr_str);
                        }
                    }
                    RegisterResponse::Failed { reason } => {
                        error!("Failed to register: {}", reason);
                    }
                }
            }
        });
        handles.push(handle);
    }
    for h in handles {
        h.await.unwrap();
    }
    return Ok(());
}

pub async fn process(mut socket: TcpStream) {
    socket.write_all(b"123123123\n").await.unwrap();
    // socket.write_i64(12345i64).await.unwrap();
}
