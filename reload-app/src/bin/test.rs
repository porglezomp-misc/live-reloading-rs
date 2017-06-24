extern crate reload_api;

use std::thread;
use std::time::Duration;

use reload_api::ShouldQuit;


fn main() {
    let mut app = reload_api::App::new("target/debug/libreloadapp.dylib")
        .expect("Should load!");
    'main: loop {
        if app.update() == ShouldQuit::Yes {
            break 'main;
        }
        thread::sleep(Duration::from_secs(1));
        app.reload().expect("Should safely reload!");
    }
}
