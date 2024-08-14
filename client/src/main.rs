mod services;

use yew::prelude::*;

struct LanBetView {
    num: Option<i32>,
    wager_data: Option<Vec<common::Wager>>
}

pub enum LanBetViewMessage {
    NewNum(i32),
    NewBetData(Vec<common::Wager>),
}

impl Component for LanBetView {
    type Message = LanBetViewMessage;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let num_cb = ctx.link().callback(LanBetViewMessage::NewNum);
        let wager_cb = ctx.link().callback(LanBetViewMessage::NewBetData);
        services::generate_new_num(num_cb);
        services::query_wager_info(wager_cb);
        Self {
            num: None,
            wager_data: None,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            LanBetViewMessage::NewNum(new_num) => {
                self.num = Some(new_num);
                true
            },
            LanBetViewMessage::NewBetData(wager_data) => {
                self.wager_data = Some(wager_data);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html!(
            <div>
                {self.num.as_ref()}
                {format!("{:?}", self.wager_data.as_ref())}
            </div>
        )
    }
}

#[function_component]
fn App() -> Html {
    let counter = use_state(|| 0);
    let onclick = {
        let counter = counter.clone();
        move |_| {
            let value = *counter + 1;
            counter.set(value);
        }
    };

    html! {
        <div>
            <button {onclick}>{ "+1" }</button>
            <p>{ *counter }</p>
        </div>
    }
}

fn main() {
    yew::Renderer::<LanBetView>::new().render();
}