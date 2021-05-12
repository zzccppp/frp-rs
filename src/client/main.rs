use std::{
    net::{SocketAddr, TcpListener},
    sync::Arc,
};

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
                let server_conn = match TcpSocket::new_v4().unwrap().connect(addr).await {
                    Ok(e) => {
                        info!("Connect Successfully on {}", target_addr_str);
                        e
                    }
                    Err(e) => {
                        error!("Cannot connect {} : {}", target_addr_str, e);
                        return;
                    }
                };
                let (server_read, server_write) = server_conn.into_split();
                let server_read = Arc::new(Mutex::new(server_read));
                let server_write = Arc::new(Mutex::new(server_write));
                loop {
                    let local_conn = match TcpSocket::new_v4().unwrap().connect(local_addr).await {
                        Ok(e) => {
                            info!("Connect Successfully on {}", local_addr_str);
                            e
                        }
                        Err(e) => {
                            error!("Cannot connect {} : {}", local_addr_str, e);
                            return;
                        }
                    };
                    let s_read = server_read.clone();
                    let s_write = server_write.clone();
                    // let (mut local_read, mut local_write) = local_conn.into_split();
                    let local_socket = Arc::new(Mutex::new(local_conn));
                    let local_socket1 = local_socket.clone();
                    let h1 = tokio::spawn(async move {
                        let socket = local_socket.clone();
                        let mut buf = [0u8; 512];
                        loop {
                            let n = { s_read.lock().await.read(&mut buf).await.unwrap() };
                            debug!("server read {}", n);
                            if n == 0 {
                                socket.lock().await.shutdown().await.unwrap();
                                return;
                            }
                            if let Err(e) = socket.lock().await.write(&buf[0..n]).await {
                                error!("{}",e);
                                return;
                            };
                        }
                    });
                    let h2 = tokio::spawn(async move {
                        let mut buf = [0u8; 512];
                        let socket = local_socket1.clone();
                        let mut n = 0;
                        loop {
                            // let n = socket.lock().await.read(&mut buf).await.unwrap();
                            {
                                if let Ok(e) = socket.lock().await.try_read(&mut buf) {
                                    n = e;
                                } else {
                                    continue;
                                }
                            }
                            debug!("local read {}", n);
                            if n == 0 {
                                socket.lock().await.shutdown().await.unwrap();
                                return;
                            }
                            {
                                s_write.lock().await.write(&buf[0..n]).await.unwrap();
                                debug!("local write {}", n);
                            }
                        }
                    });
                    h2.await.unwrap();
                    h1.await.unwrap();
                }
            };

            // let _stream = if let Ok(socket) = TcpSocket::new_v4() {
            //     let stream = socket.connect(addr).await;
            //     if let Err(e) = stream {
            //         error!("Cannot connect to {}", addr.to_string());
            //         error!("{}", e.to_string());
            //         return;
            //     }
            //     stream.unwrap();
            // } else {
            //     error!("Cannot create TcpSocket");
            // };
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
