#[macro_use]
extern crate live_reload;

mod shared_api;

use live_reload::ShouldQuit;
use shared_api::Host;

live_reload! {
    host: Host;
    state: State;
    init: init;
    reload: reload;
    update: update;
    unload: unload;
    deinit: deinit;
}

#[repr(C)]
struct State {
    counter: usize,
}

fn init(host: &mut Host, state: &mut State) {
    state.counter = 0;
    (host.print)("Init! Counter: 0.\n");
}

fn reload(host: &mut Host, state: &mut State) {
    (host.print)(&format!("Reloaded at {}.\n", state.counter));
}

fn update(host: &mut Host, state: &mut State) -> ShouldQuit {
    state.counter += 2;
    (host.print)(&format!("Counter: {}.\n", state.counter));
    ShouldQuit::No
}

fn unload(host: &mut Host, state: &mut State) {
    (host.print)(&format!("Unloaded at {}.\n", state.counter));
}

fn deinit(host: &mut Host, state: &mut State) {
    (host.print)(&format!(
        "Goodbye! Reached a final value of {}.\n",
        state.counter
    ));
}
