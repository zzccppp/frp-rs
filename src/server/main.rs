use std::{fmt::format, net::SocketAddr};

use log::{error, info, warn};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use crate::initialization::{init_logger, read_server_configuration};

pub mod initialization;
#[tokio::main]
pub async fn main() -> Result<(), ()> {
    init_logger();
    let conf = read_server_configuration().unwrap();

    let main_addr = conf.bind_ip + ":" + conf.server_port.to_string().as_str();
    let listener = TcpListener::bind(main_addr.as_str()).await.unwrap();
    info!("Server bind on address: {}", main_addr);
    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            process(stream, addr).await;
        });
    }
}

pub async fn process(mut socket: TcpStream, addr: SocketAddr) {
    info!("Receive Connection from {}", addr);
    
}
