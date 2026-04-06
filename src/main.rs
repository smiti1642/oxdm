#![allow(non_snake_case)]
use dioxus::prelude::*;

mod api;
mod components;
mod state;
mod views;

use components::{DeviceList, DevicePanel, Topbar};
use state::{Ctx, View};
use views::MainContent;

const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    dioxus::launch(App);
}

fn App() -> Element {
    let ctx = Ctx {
        devices: use_signal(Vec::new),
        selected: use_signal(|| None),
        view: use_signal(|| View::Welcome),
        scanning: use_signal(|| false),
    };
    use_context_provider(|| ctx);

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        div { class: "shell",
            Topbar {}
            div { class: "shell-body",
                DeviceList {}
                DevicePanel {}
                MainContent {}
            }
        }
    }
}
