use initialization::{init_logger, read_configuration};
use log::{error, info};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpSocket, TcpStream},
};

pub mod initialization;

#[tokio::main]
pub async fn main() -> Result<(), ()> {
    init_logger();
    let conf = read_configuration().unwrap();

    let mut handles = Vec::new();

    for client in &conf.client {
        let client = client.clone();
        let addr_str = conf.server_ip.clone() + ":" + client.remote_port.to_string().as_str();
        let addr = addr_str.parse().unwrap();
        let handle = tokio::spawn(async move {
            let stream = if let Ok(socket) = TcpSocket::new_v4() {
                let stream = socket.connect(addr).await;
                if let Err(e) = stream {
                    error!("Cannot connect to {}", addr.to_string());
                    error!("{}", e.to_string());
                    return;
                }
                stream.unwrap();
            } else {
                error!("Cannot create TcpSocket");
            };
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
