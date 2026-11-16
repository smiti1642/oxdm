pub mod grammar;
mod help;
pub mod process;
mod share_report;
mod subscriptions;
#[cfg(windows)]
mod terminal;
#[cfg(test)]
mod tests;
mod view;

use std::{collections::BTreeMap, path::PathBuf, rc::Rc, time::Duration};

use dioxus::{
    desktop::{
        tao::event::{Event, WindowEvent},
        use_window, use_wry_event_handler, WindowCloseBehaviour,
    },
    prelude::*,
};

use crate::state::{Ctx, View};

use grammar::Grammar;
pub use help::ConsoleSidebar;
use process::{Controller, LegacyHelp};
pub(crate) use subscriptions::{EventSubscriptions, ViewSubscription};
pub use view::ConsoleView;

type GrammarLoad = Option<(PathBuf, Result<Rc<Grammar>, Rc<CompatibilityError>>)>;

struct CompatibilityError {
    message: String,
    legacy: LegacyHelp,
}

#[derive(Clone, Copy)]
pub(crate) struct ConsoleState {
    executable: Signal<Option<PathBuf>>,
    requested: Signal<bool>,
    grammar: Resource<GrammarLoad>,
    controller: Signal<Controller>,
    draft: Signal<String>,
    sidebar_help: Signal<Option<bool>>,
    sidebar_collapsed: Signal<bool>,
    picking_target: Signal<bool>,
    terminal: Signal<bool>,
    presentation: Signal<RunPresentation>,
    follow_output: Signal<bool>,
    return_view: Signal<View>,
    history: Signal<Vec<String>>,
    export: Signal<Option<PathBuf>>,
}

#[derive(Default)]
struct RunPresentation {
    expanded: BTreeMap<u64, bool>,
}

impl RunPresentation {
    fn is_expanded(&self, run: u64, latest: bool, terminal: bool) -> bool {
        terminal || self.expanded.get(&run).copied().unwrap_or(latest)
    }

    fn set_expanded(&mut self, run: u64, expanded: bool, oldest_visible: u64) {
        self.expanded = self.expanded.split_off(&oldest_visible);
        self.expanded.insert(run, expanded);
    }
}

impl ConsoleState {
    pub(crate) fn enter(mut self, mut ctx: Ctx) {
        let current = *ctx.view.peek();
        if current != View::Console {
            self.return_view.set(current);
            ctx.view.set(View::Console);
        }
    }

    pub(crate) fn leave(mut self, mut ctx: Ctx) {
        let has_device = ctx
            .selected
            .peek()
            .is_some_and(|index| index < ctx.devices.peek().len());
        self.terminal.set(false);
        ctx.view
            .set(console_destination(*self.return_view.peek(), has_device));
        let _ = document::eval(
            "requestAnimationFrame(() => document.getElementById('oxdm-console-nav')?.focus());",
        );
    }

    pub(crate) fn back(mut self, ctx: Ctx) {
        if *self.picking_target.peek() {
            self.close_target_picker("oxdm-console-target");
        } else if *self.terminal.peek() {
            self.terminal.set(false);
            let _ = document::eval("requestAnimationFrame(() => document.getElementById('oxdm-console-presentation')?.focus());");
        } else {
            self.leave(ctx);
        }
    }

    fn set_draft(&mut self, text: String) {
        self.draft.set(text);
    }

    pub(crate) fn show_devices(&mut self) {
        self.picking_target.set(false);
        self.sidebar_help.set(Some(false));
        self.sidebar_collapsed.set(false);
    }

    fn help_visible(&self) -> bool {
        self.sidebar_help
            .read()
            .unwrap_or_else(|| !self.draft.read().trim().is_empty())
    }

    fn pick_target(&mut self) {
        self.picking_target.set(true);
        let _ = document::eval(
            "requestAnimationFrame(() => document.getElementById('device-list-filter')?.focus());",
        );
    }

    pub(crate) fn is_picking_target(&self) -> bool {
        *self.picking_target.peek()
    }

    fn close_target_picker(&mut self, focus: &str) {
        self.picking_target.set(false);
        let eval = document::eval(
            "const id = await dioxus.recv(); requestAnimationFrame(() => document.getElementById(id)?.focus());",
        );
        let _ = eval.send(focus);
    }

    pub(crate) fn devices_visible(&self) -> bool {
        !*self.picking_target.peek() && !*self.sidebar_collapsed.peek() && !self.help_visible()
    }
}

fn console_destination(previous: View, has_device: bool) -> View {
    if has_device || matches!(previous, View::HealthOverview) {
        previous
    } else {
        View::Welcome
    }
}

pub fn provide() -> ConsoleState {
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
    let state = use_context_provider(|| ConsoleState {
        executable,
        requested,
        grammar,
        controller,
        draft: Signal::new(String::new()),
        sidebar_help: Signal::new(None),
        sidebar_collapsed: Signal::new(false),
        picking_target: Signal::new(false),
        terminal: Signal::new(false),
        presentation: Signal::new(RunPresentation::default()),
        follow_output: Signal::new(true),
        return_view: Signal::new(View::Welcome),
        history: Signal::new(Vec::new()),
        export: Signal::new(None),
    });

    let ctx = use_context::<crate::state::Ctx>();
    use_effect(move || {
        if *ctx.view.read() != crate::state::View::Console && *state.picking_target.peek() {
            state.picking_target.clone().set(false);
        }
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
    state
}

pub fn open_with(text: String) {
    let mut state = consume_context::<ConsoleState>();
    state.set_draft(text);
    state.sidebar_help.set(None);
    state.enter(consume_context::<Ctx>());
}

pub(crate) fn select_device(mut ctx: crate::state::Ctx, index: usize) {
    ctx.selected.set(Some(index));
    if *ctx.view.peek() == crate::state::View::Console {
        let mut state = consume_context::<ConsoleState>();
        if *state.picking_target.peek() {
            state.close_target_picker("oxdm-console-input");
        }
    } else {
        ctx.view.set(crate::state::View::DeviceSettings);
    }
}

pub(crate) fn event_subscriptions() -> EventSubscriptions {
    consume_context::<ConsoleState>()
        .controller
        .peek()
        .event_subscriptions()
}
