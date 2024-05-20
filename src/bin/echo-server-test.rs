use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[tokio::main]
async fn main() {
    let mut connection = TcpStream::connect("127.0.0.1:6379").await.unwrap();
    connection.write_all(b"echo").await.unwrap();
    let mut buf = [0 as u8; 1024];
    connection.read(&mut buf).await.unwrap();
    println!("{}", String::from_utf8(buf.to_vec()).unwrap());
}
