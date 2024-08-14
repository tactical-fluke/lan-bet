use std::time::Duration;
use anyhow::bail;
use yew::platform::spawn_local;
use yew::platform::time::sleep;
use yew::Callback;
use common::network::{Connection, Packet, Request, Response};
use common::network::Request::Login;
use common::User;


pub fn query_wager_info(data_callback: Callback<Vec<common::Wager>>) {
    use common::network::*;
    spawn_local(async move {
        let mut connection = common::network::Connection::connect("127.0.0.1:6379").await.unwrap();
        let _user = login(&mut connection).await.unwrap();
        loop {
            connection.send(Packet::RequestPacket(Request::WagerData)).await.unwrap();
            let response = connection.read().await.unwrap();
            if let Packet::ResponsePacket(Response::WagerData(wager_data)) = response {
                data_callback.emit(wager_data)
            }
            sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn login(connection: &mut Connection) -> anyhow::Result<User> {
    connection.send(Packet::RequestPacket(Login { user: "aidan".into() })).await.unwrap();
    let response = connection.read().await?;
    if let Packet::ResponsePacket(Response::SuccessfulLogin {username, balance}) = response {
        Ok(User {
            name: username,
            balance
        })
    } else {
        bail!("malformed response: {:?}", response);
    }
}

pub fn generate_new_num(num_callback: Callback<i32>) {
    spawn_local(async move {
        let mut val = 1;
        loop {
            num_callback.emit(val);
            sleep(Duration::from_secs(2)).await;
            val += 1;
        }
    })
}
