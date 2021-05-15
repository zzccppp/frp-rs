use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    str::FromStr,
    sync::Arc,
    time::SystemTime,
};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use initialization::Config;
use log::{debug, error, info, warn};
use register::ConnectionState;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, UnboundedSender},
        oneshot, Mutex,
    },
};
use transport::{process_forward_bytes, ForwardPackage};
use uuid::Uuid;

use crate::{
    initialization::{init_logger, read_server_configuration},
    register::{ClientRegisterMessage, RegisterResponse},
};

pub mod initialization;
pub mod register;
pub mod transport;

#[tokio::main]
pub async fn main() -> Result<(), ()> {
    init_logger();
    let conf = read_server_configuration().unwrap();
    let connection_state = Arc::new(Mutex::new(HashMap::<String, ConnectionState>::new()));

    let main_addr = conf.bind_ip.clone() + ":" + conf.server_port.to_string().as_str();
    let listener = TcpListener::bind(main_addr.as_str()).await.unwrap();
    info!("Server bind on address: {}", main_addr);
    loop {
        let conf = conf.clone();
        let db = connection_state.clone();
        let (stream, addr) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            process(stream, addr, conf, db).await;
        });
    }
}

pub async fn process(
    mut socket: TcpStream,
    addr: SocketAddr,
    conf: Config,
    db: Arc<Mutex<HashMap<String, ConnectionState>>>,
) {
    info!("Receive Connection from {}", addr);
    let mut recv_buf = [0u8; 512];
    let size = socket.read(&mut recv_buf).await.unwrap();
    let client: ClientRegisterMessage = bincode::deserialize(&recv_buf[0..size]).unwrap();
    info!("Received the register information: {:?}", client);
    let find = conf
        .client
        .iter()
        .find(|x| x.name == client.name && x.secret_key == client.secret);
    if let None = find {
        info!("Client Register Failed: {:?}", client);
        let response = RegisterResponse::Failed {
            reason: "Invalid register configuration.".to_string(),
        };
        let b = bincode::serialize(&response).unwrap();
        match socket.write(b.as_slice()).await {
            Ok(e) => {
                debug!("Write register failed info, size of bytes: {}", e);
            }
            Err(e) => {
                error!("Write Register Response Error: {}", e);
            }
        }
        return;
    }
    let find = find.unwrap();
    let uuid = Uuid::new_v4();
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    db.lock().await.insert(
        uuid.to_string(),
        ConnectionState {
            last_heart_beat: 0, //TODO:
            register_time: since_the_epoch.as_millis(),
            name: client.name,
        },
    );
    info!(
        "Client named {} register successfully, uuid: {}",
        find.name,
        uuid.to_string()
    );
    let response = RegisterResponse::Succ {
        uuid: uuid.to_string(),
    };
    let b = bincode::serialize(&response).unwrap();

    let forward_addr = conf.bind_ip + ":" + find.port.to_string().as_str();
    let forward_listener = TcpListener::bind(&forward_addr).await;
    if let Err(e) = forward_listener {
        error!("Failed to bind on {} to forward data: {}", forward_addr, e);
        return;
    }
    let forward_listener = forward_listener.unwrap();
    info!("Listen on {} to forward data", forward_addr.to_string());
    match socket.write(b.as_slice()).await {
        Ok(e) => {
            debug!("Write register succ info, size of bytes: {}", e);
        }
        Err(e) => {
            error!("Write Register Response Error: {}", e);
        }
    }
    loop {
        let (st, ad) = forward_listener.accept().await.unwrap();
        let mut forward_socket = st;

        if ad.ip().eq(&addr.ip()) {
            let mut buf = [0u8; 128];
            let re = forward_socket.read(&mut buf).await;
            if let Ok(n) = re {
                let receive_uuid_str: String = bincode::deserialize(&buf[0..n]).unwrap();
                let receive_uuid = Uuid::parse_str(receive_uuid_str.as_str());
                if let Ok(ruuid) = receive_uuid {
                    if uuid.eq(&ruuid) {
                        info!(
                            "Client Connect forward port successfully with uuid: {} , name: {}",
                            receive_uuid_str, find.name
                        );
                        forward_socket
                            .write_all(bincode::serialize("OK").unwrap().as_ref())
                            .await
                            .unwrap();
                        handle_forawrd_connection(forward_socket, forward_listener).await;
                    } else {
                        info!(
                            "Client send uuid: {} isn't match {}",
                            receive_uuid_str,
                            uuid.to_string()
                        );
                        break;
                    }
                } else {
                    info!(
                        "Client send uuid: {} isn't match {}",
                        receive_uuid_str,
                        uuid.to_string()
                    );
                    break;
                }
            } else {
                forward_socket
                    .write(b"Service Unavaliable.\n")
                    .await
                    .unwrap();
                forward_socket.shutdown().await.unwrap();
            }
            break;
        } else {
            forward_socket
                .write(b"Service Unavaliable.\n")
                .await
                .unwrap();
            forward_socket.shutdown().await.unwrap();
        }
    }
}

async fn handle_forawrd_connection(
    mut forward_socket: TcpStream,
    mut forward_listener: TcpListener,
) {
    let mut map: HashMap<Uuid, UnboundedSender<BytesMut>> = HashMap::new();
    let mut map = Arc::new(Mutex::new(map));

    let (sender, mut receiver) = mpsc::unbounded_channel::<ForwardPackage>();

    let (mut read, mut write) = forward_socket.into_split();

    tokio::spawn(async move {
        // this task is used for sent the packets
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

    let mm = map.clone();

    tokio::spawn(async move {
        let mut buf = BytesMut::with_capacity(128);
        let map = mm;
        loop {
            let re = read.read_buf(&mut buf).await;
            if let Ok(n) = re {
                if n != 0 {
                    while let Some(packet) = process_forward_bytes(&mut buf) {
                        debug!("Receive the packet {:?}", packet);
                        match packet {
                            ForwardPackage::Transport { uuid, data } => {
                                if let Some(e) = map.lock().await.get(&uuid) {
                                    if let Err(e) = e.send(data) {
                                        error!("{}", e);
                                    };
                                } else {
                                    error!("Cannot find the receiver");
                                }
                            }
                            ForwardPackage::HeartBeat { state } => {}
                        }
                    }
                }
            } else {
                break;
            }
        }
    });

    loop {
        let (stream, addr) = forward_listener.accept().await.unwrap();
        handle_transport(stream, map.clone(), sender.clone()).await;
    }
}

async fn handle_transport(
    stream: TcpStream,
    map: Arc<Mutex<HashMap<Uuid, UnboundedSender<BytesMut>>>>,
    sender: UnboundedSender<ForwardPackage>,
) {
    let uuid = Uuid::new_v4();

    let (mut read, mut write) = stream.into_split();

    tokio::spawn(async move {
        let uuid = uuid.clone();
        let mut buf = [0u8; 512];
        loop {
            match read.read(&mut buf).await {
                Ok(n) => {
                    if n != 0 {
                        let mut bytes = BytesMut::with_capacity(512);
                        bytes.put(&buf[0..n]);
                        let packet = ForwardPackage::Transport { uuid, data: bytes };
                        sender.send(packet).unwrap();
                    }
                }
                Err(e) => {
                    error!("{}", e);
                    break;
                }
            }
        }
    });
    let (tx, mut rx) = mpsc::unbounded_channel();
    map.lock().await.insert(uuid, tx);
    tokio::spawn(async move {
        while let Some(mut data) = rx.recv().await {
            while data.has_remaining() {
                write.write_buf(&mut data).await.unwrap();
            }
        }
    });
}
