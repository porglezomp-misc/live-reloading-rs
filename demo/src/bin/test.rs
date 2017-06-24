extern crate live_reload;

use std::thread;
use std::time::Duration;

use live_reload::ShouldQuit;


fn main() {
    let mut app = live_reload::App::new("target/debug/libreloadapp.dylib")
        .expect("Should load!");
    loop {
        if app.update() == ShouldQuit::Yes {
            break;
        }
        thread::sleep(Duration::from_secs(1));
        app.reload().expect("Should safely reload!");
    }
}
