use lan_bet::network::*;
use std::io::ErrorKind;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        let (connection, _) = listener.accept().await.unwrap();

        tokio::spawn(async move {
            let connection = Connection::from_tcp_stream(connection);
            handle_connection(connection).await;
        });
    }
}

async fn handle_connection(mut connection: Connection) {
    let user = handle_login(&mut connection).await;
    if let Ok(username) = user {
        connection
            .send(Packet::ResponsePacket(Response::Ok(RequestResponse::None)))
            .await
            .unwrap();
        handle_client(username, connection).await;
    } else {
        connection
            .send(Packet::ResponsePacket(Response::Error))
            .await
            .unwrap();
    }
}

async fn handle_login(connection: &mut Connection) -> tokio::io::Result<String> {
    let packet = connection.read().await.unwrap();
    if let Packet::RequestPacket(request) = packet {
        match request {
            Request::Login { user } => Ok(user),
            _ => {
                let error = tokio::io::Error::new(ErrorKind::Other, "bad login");
                Err(error)
            }
        }
    } else {
        Err(tokio::io::Error::new(
            ErrorKind::InvalidData,
            "invalid request at login",
        ))
    }
}

async fn handle_client(username: String, mut connection: Connection) {
    loop {
        let packet = connection.read().await;
        if let Ok(packet) = packet {
            if let Packet::RequestPacket(request) = packet {
                match request {
                    Request::Login { user: _ } => {
                        connection
                            .send(Packet::ResponsePacket(Response::Error))
                            .await
                            .unwrap();
                        break; //relogin, no
                    }
                    Request::WhoAmI => {
                        connection
                            .send(Packet::ResponsePacket(Response::Ok(
                                RequestResponse::WhoAmI(username.clone()),
                            )))
                            .await
                            .unwrap();
                    }
                }
            }
        } else {
            eprintln!("{:?}", packet);
            break;
        }
    }
}
