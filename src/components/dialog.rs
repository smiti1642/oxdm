#![allow(non_snake_case)]
use crate::state::Ctx;
use dioxus::prelude::*;

#[component]
pub fn ConfirmDialogModal() -> Element {
    let ctx = use_context::<Ctx>();
    let mut dialog_sig = ctx.dialog;
    let dialog = ctx.dialog.read();

    let Some(dlg) = dialog.as_ref() else {
        return rsx! {};
    };

    let title = dlg.title.clone();
    let message = dlg.message.clone();
    let confirm_label = dlg.confirm_label.clone();
    let cancel_label = dlg.cancel_label.clone();
    let dangerous = dlg.dangerous;
    let on_confirm = dlg.on_confirm;

    let confirm_class = if dangerous {
        "btn btn-md btn-danger"
    } else {
        "btn btn-md btn-primary"
    };

    rsx! {
        div {
            class: "dialog-overlay",
            tabindex: "-1",
            onmousedown: move |_| dialog_sig.set(None),
            onkeydown: move |evt: KeyboardEvent| {
                if evt.key() == Key::Escape {
                    dialog_sig.set(None);
                }
            },

            div {
                class: "dialog",
                onmousedown: |e| e.stop_propagation(),

                div { class: "dialog-header",
                    span { class: "dialog-title", "{title}" }
                }
                div { class: "dialog-body",
                    p { class: "dialog-message", "{message}" }
                }
                div { class: "dialog-footer",
                    button {
                        class: "btn btn-md btn-ghost",
                        onclick: move |_| dialog_sig.set(None),
                        "{cancel_label}"
                    }
                    button {
                        class: confirm_class,
                        onclick: move |_| {
                            on_confirm.call(());
                            dialog_sig.set(None);
                        },
                        "{confirm_label}"
                    }
                }
            }
        }
    }
}
