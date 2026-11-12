pub mod grammar;
pub mod process;
mod subscriptions;
#[cfg(test)]
mod tests;
mod view;

use std::{path::PathBuf, rc::Rc, time::Duration};

use dioxus::{
    desktop::{
        tao::event::{Event, WindowEvent},
        use_window, use_wry_event_handler, WindowCloseBehaviour,
    },
    prelude::*,
};
use grammar::Grammar;
use process::{Controller, LegacyHelp};
pub(crate) use subscriptions::{EventSubscriptions, ViewSubscription};
pub use view::ConsoleView;

type GrammarLoad = Option<(PathBuf, Result<Rc<Grammar>, Rc<CompatibilityError>>)>;

struct CompatibilityError {
    message: String,
    legacy: LegacyHelp,
}

#[derive(Clone, Copy)]
struct ConsoleState {
    executable: Signal<Option<PathBuf>>,
    requested: Signal<bool>,
    grammar: Resource<GrammarLoad>,
    controller: Signal<Controller>,
    draft: Signal<String>,
    history: Signal<Vec<String>>,
    export: Signal<Option<PathBuf>>,
}

pub fn provide() {
    let executable = use_signal(process::default_executable);
    let requested = use_signal(|| false);
    let grammar = use_resource(move || {
        let path = requested
            .read()
            .then(|| executable.read().clone())
            .flatten();
        async move {
            let path = path?;
            let result = match process::load_grammar(&path).await {
                Ok(grammar) => Ok(Rc::new(grammar)),
                Err(message) => Err(Rc::new(CompatibilityError {
                    message,
                    legacy: process::load_legacy_help(&path).await,
                })),
            };
            Some((path, result))
        }
    });
    let controller = use_signal(Controller::default);
    use_context_provider(|| ConsoleState {
        executable,
        requested,
        grammar,
        controller,
        draft: Signal::new(String::new()),
        history: Signal::new(Vec::new()),
        export: Signal::new(None),
    });

    let window = use_window();
    let mut closing = false;
    use_wry_event_handler(move |event, _| {
        if closing {
            if !controller.peek().is_active() {
                closing = false;
                window.set_close_behavior(WindowCloseBehaviour::WindowCloses);
                window.close();
            }
            return;
        }
        if matches!(event, Event::WindowEvent { event: WindowEvent::CloseRequested, window_id, .. }
            if *window_id == window.id())
            && controller.peek().is_active()
        {
            closing = true;
            let controller = controller.peek().clone();
            window.set_close_behavior(WindowCloseBehaviour::WindowHides);
            controller.stop();
            let native_window = window.window.clone();
            // A hidden WebView can suspend the virtual DOM. Wait on Tokio
            // and wake the native event loop once the sidecar has exited.
            tokio::spawn(async move {
                while controller.is_active() {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                native_window.request_redraw();
            });
        }
    });
}

pub fn open_with(text: String) {
    let mut state = consume_context::<ConsoleState>();
    state.draft.set(text);
    let mut ctx = consume_context::<crate::state::Ctx>();
    ctx.view.set(crate::state::View::Console);
}

pub(crate) fn event_subscriptions() -> EventSubscriptions {
    consume_context::<ConsoleState>()
        .controller
        .peek()
        .event_subscriptions()
}
