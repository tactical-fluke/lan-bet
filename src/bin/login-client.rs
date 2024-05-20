use lan_bet::*;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let connection = TcpStream::connect("127.0.0.1:6379").await.unwrap();
    let mut  connection = Connection::from_tcp_stream(connection);
    connection.send(Packet::RequestPacket(Request::Login { user: "aidan".into() })).await.unwrap();
    let response = connection.read().await.unwrap();
    if response == Packet::ResponsePacket(Response::Ok(RequestResponse::None)) {
        println!("success!");
    } else {
        println!("oh no!")
    }
    connection.send(Packet::RequestPacket(Request::WhoAmI)).await.unwrap();
    let response = connection.read().await.unwrap();
    if let Packet::ResponsePacket(Response::Ok(RequestResponse::WhoAmI(username))) = response {
        println!("username: {}", username);
    } else {
        println!("oh no! {:?}", response);
    }
}