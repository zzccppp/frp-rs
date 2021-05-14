use std::{
    collections::HashMap,
    net::{SocketAddr, TcpListener},
    sync::Arc,
};

use crate::register::RegisterResponse;
use bytes::{Buf, BytesMut};
use initialization::{init_logger, read_configuration};
use log::{debug, error, info};
use register::register;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpSocket, TcpStream},
    sync::{
        mpsc::{self, UnboundedSender},
        Mutex,
    },
};
use transport::{process_forward_bytes, ForwardPackage};
use uuid::Uuid;

pub mod initialization;
pub mod register;
pub mod transport;

#[tokio::main]
pub async fn main() -> Result<(), ()> {
    init_logger();
    let conf = read_configuration().unwrap();

    let mut handles = Vec::new();

    for client in &conf.client {
        let client = client.clone();
        let target_addr_str =
            conf.server_ip.clone() + ":" + client.remote_port.to_string().as_str();
        let target_addr: SocketAddr = target_addr_str.parse().unwrap();
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
                        let uuid_bin = bincode::serialize(&uuid).unwrap();
                        let conn = TcpSocket::new_v4().unwrap().connect(target_addr).await;
                        if let Ok(mut stream) = conn {
                            stream.write(uuid_bin.as_slice()).await.unwrap();
                            let mut buf = [0u8; 64];
                            let n = stream.read(&mut buf).await.unwrap();
                            let resp_str: String = bincode::deserialize(&buf[0..n]).unwrap();
                            if resp_str.eq("OK") {
                                process(stream, local_addr).await;
                            } else {
                                error!("Connect Failed!");
                            }
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

pub async fn process(mut socket: TcpStream, local_addr: SocketAddr) {
    let (mut read, mut write) = socket.into_split();
    let (sender, mut receiver) = mpsc::unbounded_channel::<ForwardPackage>();
    let h1 = tokio::spawn(async move {
        let mut buf = BytesMut::with_capacity(128);
        let mut map: HashMap<Uuid, UnboundedSender<BytesMut>> = HashMap::new();
        loop {
            let re = read.read_buf(&mut buf).await;
            if let Ok(n) = re {
                if n != 0 {
                    if let Some(packet) = process_forward_bytes(&mut buf) {
                        debug!("Receive packets from proxy server : {:?}", packet);
                        match packet {
                            transport::ForwardPackage::Transport { uuid, data } => {
                                let (tx, mut rx) = mpsc::unbounded_channel();
                                if map.contains_key(&uuid) {
                                    if let Err(e) = map.get(&uuid).unwrap().send(data) {
                                        error!("forwarding data {} error", e);
                                    }
                                } else {
                                    map.insert(uuid, tx);
                                    map.get(&uuid).unwrap().send(data).unwrap();
                                    //establish the conn to local addr
                                    let conn =
                                        TcpSocket::new_v4().unwrap().connect(local_addr).await;
                                    if let Err(e) = conn {
                                        error!(
                                            "Cannot establish the conn to addr:{}, {}",
                                            local_addr.to_string(),
                                            e
                                        );
                                        continue;
                                    }
                                    let (mut read, mut write) = conn.unwrap().into_split();
                                    debug!("Connect to local: {} succ", local_addr);
                                    tokio::spawn(async move {
                                        while let Some(mut data) = rx.recv().await {
                                            while data.has_remaining() {
                                                info!("write data to:");
                                                write.write_buf(&mut data).await.unwrap();
                                            }
                                        }
                                    });
                                    let sender = sender.clone();
                                    tokio::spawn(async move {
                                        loop {
                                            let mut buf = BytesMut::with_capacity(512);
                                            match read.read_buf(&mut buf).await {
                                                Ok(n) => {
                                                    if n != 0 {
                                                        sender
                                                            .send(ForwardPackage::Transport {
                                                                uuid,
                                                                data: buf,
                                                            })
                                                            .unwrap();
                                                    }
                                                }
                                                Err(e) => {
                                                    error!("{}", e);
                                                    break;
                                                }
                                            }
                                        }
                                    });
                                }
                            }
                            transport::ForwardPackage::HeartBeat { state } => {}
                        }
                    };
                }
            } else {
                break;
            }
        }
    });
    let h2 = tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Some(package) => {
                    debug!("Send the package: {:?}", package);
                    let bin = bincode::serialize(&package).unwrap();
                    let packet_length = bin.len();
                    //send the packet magic number (java .class magic number), and the data's length
                    write.write_u32(0xCAFEBABE).await.unwrap(); //TODO: handle the error here
                    write.write_u32(packet_length as u32).await.unwrap();
                    if let Ok(_) = write.write_all(bin.as_slice()).await {
                    } else {
                        break;
                    };
                }
                None => {
                    break;
                }
            }
        }
    });
    h1.await.unwrap();
    h2.await.unwrap();
}
