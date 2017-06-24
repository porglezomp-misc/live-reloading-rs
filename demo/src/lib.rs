#[macro_use] extern crate live_reload;

use std::io::Write;

use live_reload::ShouldQuit;

live_reload! {
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

fn init(state: &mut State) {
    println!("Init!");
    state.counter = 0;
}

fn reload(_state: &mut State) {
}

fn update(state: &mut State) -> ShouldQuit {
    state.counter += 1;
    print!("Update! {:04}\r", state.counter);
    std::io::stdout().flush().unwrap();
    ShouldQuit::No
}

fn unload(_state: &mut State) {
}

fn deinit(state: &mut State) {
    println!("Deinit! Final count was: {}", state.counter);
}
