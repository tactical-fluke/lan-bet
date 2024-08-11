use std::time::Duration;
use yew::platform::spawn_local;
use yew::platform::time::sleep;
use yew::Callback;
use common::network::Packet;


pub fn query_wager_info(mut connection: common::network::Connection, data_callback: Callback<Vec<common::Wager>>) {
    use common::network::*;
    spawn_local(async move {
        loop {
            connection.send(Packet::RequestPacket(Request::WagerData)).await.unwrap();
            let response = connection.read().await.unwrap();
            if let Packet::ResponsePacket(Response::WagerData(wager_data)) = response {
                data_callback.emit(wager_data)
            }
            sleep(std::time::Duration::from_secs(1)).await;
        }
    });
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
