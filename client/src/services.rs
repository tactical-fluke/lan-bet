use chrono::{DateTime, Local};
use futures::{Stream, StreamExt};
use yew::platform::pinned::mpsc::UnboundedSender;
use yew::platform::spawn_local;
use yew::platform::time::{interval, sleep};
use yew::{AttrValue, Callback};


pub fn query_wager_info(mut connection: common::network::Connection, data_callback: Callback<Vec<common::Wager>>) {
    use common::network::*;
    spawn_local(async move {
        loop {
            connection.send(Packet::RequestPacket(Request::WagerData)).await.unwrap();
            let response = connection.read().await.unwrap();
            if let Response::WagerData(wager_data) = response{
                data_callback.emit(wager_data)
            }
            sleep(std::time::Duration::from_secs(1)).await;
        }
    });
}
