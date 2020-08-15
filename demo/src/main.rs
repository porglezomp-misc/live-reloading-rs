extern crate live_reload;

mod shared_api;

use std::io::Write;

use std::thread;
use std::time::Duration;

use live_reload::ShouldQuit;
use shared_api::Host;

type App = live_reload::Reloadable<shared_api::Host>;

fn print(msg: &str) {
    print!("{}", msg);
    let _ = std::io::stdout().flush();
}

fn main() {
    let mut app =
        App::new("target/debug/libreloadable.dylib", Host { print }).expect("Should load!");
    loop {
        if app.update() == ShouldQuit::Yes {
            break;
        }
        thread::sleep(Duration::from_secs(1));
        app.reload().expect("Should safely reload!");
    }
}
