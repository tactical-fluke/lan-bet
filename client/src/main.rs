mod services;

use yew::prelude::*;

struct LanBetView {
    num: Option<i32>,
}

pub enum LanBetViewMessage {
    NewNum(i32),
}

impl Component for LanBetView {
    type Message = LanBetViewMessage;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let num_cb = ctx.link().callback(LanBetViewMessage::NewNum);
        services::generate_new_num(num_cb);
        Self {
            num: None,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            LanBetViewMessage::NewNum(new_num) => {
                self.num = Some(new_num);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html!(
            <div>
                {self.num.as_ref()}
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