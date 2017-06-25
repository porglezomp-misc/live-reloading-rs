# Live Reload

[![License](https://img.shields.io/crates/l/live-reload.svg)](https://opensource.org/licenses/Zlib/)
[![Version number](https://img.shields.io/crates/v/live-reload.svg)](https://crates.io/crates/live-reload/)
[![Documentation](https://docs.rs/live-reload/badge.svg)](https://docs.rs/live-reload/)
[![Travis CI](https://travis-ci.org/porglezomp-misc/live-reloading-rs.svg)](https://travis-ci.org/porglezomp-misc/live-reloading-rs/)

This is inspired by the article ["Interactive Programming in C"][] by Chris
Wellons, and the video ["Loading Game Code Dynamically"][] from Handmade Hero by
Casey Muratori.

The general idea is that your main host program is a wrapper around a dynamic
library that does all the interesting work of your game. This means that you can
simply reload the library while the game is still running, and have your game
update live. As a consequence however, you can't have any global state in your
library, everything must be owned by the host in order to avoid getting unloaded
with the library.

["Interactive Programming in C"]: http://nullprogram.com/blog/2014/12/23/
["Loading Game Code Dynamically"]: https://www.youtube.com/watch?v=WMSBRk5WG58

## Getting Started

Add this to your `Cargo.toml`:

```toml
[lib]
name = "<your library name>"
crate-type = ["cdylib"]

[dependencies]
live-reload = "0.1"
```

To do live reloading, you'll need to build both a library and a binary. Inside
the host binary, you'll want to use `live-reload` with:

```rust
extern crate live_reload;
```

In the library, you need to use a macro to declare the live-reloading API, so
you need:

```rust
#[macro_use] extern crate live_reload;
```

See the [Documentation](https://docs.rs/live-reload/) for instructions on how to
use the library to create the library and host program.
