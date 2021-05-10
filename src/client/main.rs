use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};

use log::info;

#[tokio::main]
pub async fn main() -> Result<(), ()> {
    let env = env_logger::Env::default()
        .filter_or("MY_LOG_LEVEL", "debug")
        .write_style_or("MY_LOG_STYLE", "always");
    env_logger::init_from_env(env);

    let listener = TcpListener::bind("127.0.0.1:12000").await.unwrap();
    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            process(stream).await;
        });
    }
    //return Ok(());
}

pub async fn process(mut socket: TcpStream) {
    socket.write_all(b"123123123\n").await.unwrap();
    // socket.write_i64(12345i64).await.unwrap();
}
