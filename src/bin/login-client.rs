use lan_bet::network::*;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let connection = TcpStream::connect("127.0.0.1:6379").await.unwrap();
    let mut connection = Connection::from_tcp_stream(connection);
    connection
        .send(Packet::RequestPacket(Request::Login {
            user: "aidan".into(),
        }))
        .await
        .unwrap();
    let response = connection.read().await.unwrap();
    if response == Packet::ResponsePacket(Response::None) {
        println!("success!");
    } else {
        println!("oh no!")
    }
    connection
        .send(Packet::RequestPacket(Request::WhoAmI))
        .await
        .unwrap();
    let response = connection.read().await;
    dbg!(&response);
    if let Ok(Packet::ResponsePacket(Response::WhoAmI(username))) = response {
        println!("username: {}", username);
    } else {
        println!("oh no! {:?}", response);
    }

    connection
        .send(Packet::RequestPacket(Request::WagerData))
        .await
        .unwrap();
    let response = connection.read().await;
    dbg!(&response);

    match response.unwrap() {
        Packet::ResponsePacket(response) => {
            if let Response::WagerData(data) = response {
                let resolved_bet = data.first().unwrap();
                let winning_option = resolved_bet.options.first().unwrap();
                connection.send(Packet::RequestPacket(Request::ResolveWager { 
                    wager_id: resolved_bet.id.clone(), 
                    winning_option_id: winning_option.id.clone() 
                })).await.unwrap();
            } else {}
        },
        _ => {}
    }
}
