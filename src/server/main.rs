use std::{collections::HashMap, net::SocketAddr, str::FromStr, sync::Arc, time::SystemTime};

use initialization::Config;
use log::{debug, error, info, warn};
use register::ConnectionState;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{oneshot, Mutex},
};
use uuid::Uuid;

use crate::{
    initialization::{init_logger, read_server_configuration},
    register::{ClientRegisterMessage, RegisterResponse},
};

pub mod initialization;
pub mod register;

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
    let mut is_client_conn = false;
    loop {
        let (st, ad) = forward_listener.accept().await.unwrap();
        let mut forward_socket: Arc<Mutex<TcpStream>> = Arc::new(Mutex::new(st));

        if ad.ip().eq(&addr.ip()) {
            let mut buf = [0u8; 128];
            let re = forward_socket.lock().await.read(&mut buf).await;
            if let Ok(n) = re {
                let receive_uuid_str: String = bincode::deserialize(&buf[0..n]).unwrap();
                let receive_uuid = Uuid::parse_str(receive_uuid_str);
                if let Ok(ruuid) = receive_uuid {
                    if (receive_uuid.eq(&uuid)) {
                        is_client_conn = true;
                        handle_forawrd_connection();
                    } else {
                        info!(
                            "Client send uuid: {} isn't match {}",
                            receive_uuid_str,
                            uuid.to_string()
                            break;
                        );
                    }
                } else {
                    info!(
                        "Client send uuid: {} isn't match {}",
                        receive_uuid_str,
                        uuid.to_string()
                        break;
                    );
                }
            } else {
                forward_socket
                    .lock()
                    .await
                    .write(b"Service Unavaliable.\n")
                    .await
                    .unwrap();
                forward_socket.lock().await.shutdown().await.unwrap();
            }

            is_client_conn = true;
            break;
        } else {
            forward_socket
                .lock()
                .await
                .write(b"Service Unavaliable.\n")
                .await
                .unwrap();
            forward_socket.lock().await.shutdown().await.unwrap();
        }
    }

    loop {
        let (mut st, ad) = forward_listener.accept().await.unwrap();
        if !is_client_conn {
            st.write(b"Service Unavaliable.\n").await.unwrap();
            st.shutdown().await.unwrap();
        } else {
            let forward_read = forward_socket.clone();
            let forward_write = forward_socket.clone();
            // let (mut read, mut write) = st.into_split();
            let socket = Arc::new(Mutex::new(st));
            let socket1 = socket.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 512];
                let socket = socket.clone();
                let mut n = 0;
                loop {
                    // let n = read.read(&mut buf).await.unwrap();
                    {
                        if let Ok(e) = socket.lock().await.try_read(&mut buf) {
                            n = e;
                        } else {
                            continue;
                        }
                    }
                    debug!("Request read : {}", n);
                    if n == 0 {
                        socket.lock().await.shutdown().await.unwrap();
                        break;
                    }
                    {
                        // let mut x = socket.lock().await;
                        // x.write(&buf[0..n]).await.unwrap();
                        forward_write.lock().await.write(&buf[0..n]).await.unwrap();
                    }
                }
            });
            // let socket = forward_socket.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 512];
                let socket = socket1.clone();
                loop {
                    let mut n = 0;
                    {
                        // n = forward_read.lock().await.read(&mut buf).await.unwrap();
                        if let Ok(e) = forward_read.lock().await.try_read(&mut buf) {
                            n = e;
                        } else {
                            continue;
                        }
                    }
                    debug!("Client read : {}", n);
                    if n == 0 {
                        socket.lock().await.shutdown().await.unwrap();
                        break;
                    }
                    // if let Err(e) = write.write(&buf[0..n]).await {
                    //     error!("{}", e);
                    //     return;
                    // };
                    {
                        socket.lock().await.write(&buf[0..n]).await.unwrap();
                    }
                }
            });
        }
    }
}
