#![allow(non_snake_case)]
use crate::api;
use dioxus::prelude::*;

#[component]
pub fn CameraDetail(addr: String) -> Element {
    let info = use_resource(move || {
        let addr = addr.clone();
        async move {
            // Parse host:port from the xaddr URL
            api::get_device_info(&addr, None, None).await
        }
    });

    rsx! {
        div { class: "p-6 max-w-3xl mx-auto",
            Link { to: crate::Route::Home {},
                class: "text-blue-500 text-sm mb-4 inline-block hover:underline",
                "← Back"
            }
            match &*info.read_unchecked() {
                None => rsx! { p { class: "text-gray-400 mt-6", "Loading…" } },
                Some(Err(e)) => rsx! { p { class: "text-red-500 mt-6", "{e}" } },
                Some(Ok(dev)) => rsx! {
                    h2 { class: "text-xl font-bold mb-4 dark:text-white", "{dev.manufacturer} {dev.model}" }
                    table { class: "w-full text-sm border-collapse",
                        Row { label: "Manufacturer", value: dev.manufacturer.clone() }
                        Row { label: "Model",        value: dev.model.clone() }
                        Row { label: "Firmware",     value: dev.firmware_version.clone() }
                        Row { label: "Serial",       value: dev.serial_number.clone() }
                        Row { label: "Hardware ID",  value: dev.hardware_id.clone() }
                    }
                },
            }
        }
    }
}

#[component]
fn Row(label: &'static str, value: String) -> Element {
    rsx! {
        tr { class: "border-b border-gray-200 dark:border-gray-700",
            td { class: "py-2 pr-4 font-medium text-gray-600 dark:text-gray-400 w-1/3", "{label}" }
            td { class: "py-2 dark:text-white", "{value}" }
        }
    }
}

