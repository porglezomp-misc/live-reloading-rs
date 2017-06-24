#[macro_use] extern crate reload_api;

use reload_api::ShouldQuit;

reload_api! {
    state: State;
    init: init;
    load: load;
    update: update;
    unload: unload;
    deinit: deinit;
}

struct State {
    counter: usize,
}

fn init(state: &mut State) {
    println!("Init!");
    state.counter = 0;
}

fn load(_state: &mut State) {
    println!("Load!");
}

fn update(state: &mut State) -> ShouldQuit {
    state.counter += 1;
    println!("Update {}", state.counter);
    ShouldQuit::No
}

fn unload(_state: &mut State) {
    println!("Unload!");
}

fn deinit(_state: &mut State) {
    println!("Deinit!");
}
