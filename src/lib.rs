#![deny(missing_docs)]

//! A library for doing live-reloading game development.
//!
//! This is inspired by the article ["Interactive Programming in C"][] by Chris
//! Wellons, and the video ["Loading Game Code Dynamically"][] from Handmade
//! Hero by Casey Muratori.
//!
//! The general idea is that your main host program is a wrapper around a
//! dynamic library that does all the interesting work of your game. This means
//! that you can simply reload the library while the game is still running, and
//! have your game update live. As a consequence however, you can't have any
//! global state in your library, everything must be owned by the host in order
//! to avoid getting unloaded with the library.
//!
//! ["Interactive Programming in C"]: http://nullprogram.com/blog/2014/12/23/
//! ["Loading Game Code Dynamically"]: https://www.youtube.com/watch?v=WMSBRk5WG58
//!
//! Currently, this library doesn't provide a good solution for calling back
//! into the wrapper code, which you want to do for things like allocating
//! memory. That will hopefully come in a new version of the crate very soon.
//!
//! See the Host Example and Library Example sections for instructions on how to
//! build a reloadable application.
//!
//! # Host Example
//!
//! A program that hosts a reloadable library will need to load the library, and
//! then periodically reload it. The [`Reloadable`][] automatically installs a
//! filesystem watcher for you so that it knows when the library file has been
//! updated or replaced, and the [`reload`][] method will only actually perform
//! a reload if the file has changed. The core of your main loop will therefore
//! usually look something like this:
//!
//! ```rust,no_run
//! use std::thread;
//!
//! fn main() {
//!     let mut prog = live_reload::Reloadable::new("target/debug/libreload.dylib")
//!         .expect("Should successfully load");
//!     'main: loop {
//!         if prog.update() == live_reload::ShouldQuit::Yes {
//!             break 'main;
//!         }
//!         prog.reload().expect("Should successfully reload");
//!     }
//! }
//! ```
//!
//! # Library Example
//!
//! A live-reloadable library needs to register its entry-points so that the
//! host program can find them. The [`live_reload!`][] macro lets you do this
//! conveniently.
//!
//! The lifecycle of your reloadable library will happen in a few stages:
//!
//! - `init` gets called at the very beginning of the program, when the host
//!   starts for the first time.
//! - `reload` gets called on each library load, including the first time. This
//!   should be usually empty, but when you're in development, you might want to
//!   reset things here, or migrate data, or things like that. The pointer
//!   you're passed will refer to the same struct that you had when the previous
//!   library was unloaded, so it might not be properly initialized. You should
//!   try to make your struct be `#[repr(C)]`, and only add members at the end
//!   to minimize the problems of reloading.
//! - `update` gets called at the host program's discretion. You'll probably end
//!   up calling this once per frame. In addition to doing whatever work you
//!   were interested in, `update` also returns a value indicating whether the
//!   host program should quit.
//! - `unload` gets called before a library unloads. This will probably be empty
//!   even more often than `reload`, but you might need it for some debugging or
//!   data migration purpose.
//! - `deinit` gets called when the host program is actually shutting down--it's
//!   called on the drop of the [`Reloadable`][].
//!
//! Here's an example of a live-reloadable library that handles a counter.
//!
//! ```rust
//! #[macro_use] extern crate live_reload;
//! # fn main() {}
//! use live_reload::ShouldQuit;
//!
//! live_reload! {
//!     state: State;
//!     init: my_init;
//!     reload: my_reload;
//!     update: my_update;
//!     unload: my_unload;
//!     deinit: my_deinit;
//! }
//!
//! struct State {
//!     counter: u64,
//! }
//!
//! fn my_init(state: &mut State) {
//!     state.counter = 0;
//!     println!("Init! Counter: 0.");
//! }
//!
//! fn my_reload(state: &mut State) {
//!     println!("Reloaded at {}.", state.counter);
//! }
//!
//! fn my_update(state: &mut State) -> ShouldQuit {
//!     state.counter += 1;
//!     println!("Counter: {}.", state.counter);
//!     ShouldQuit::No
//! }
//!
//! fn my_unload(state: &mut State) {
//!     println!("Unloaded at {}.", state.counter);
//! }
//!
//! fn my_deinit(state: &mut State) {
//!     println!("Goodbye! Reached a final value of {}.", state.counter);
//! }
//! ```
//!
//! [`Reloadable`]: struct.Reloadable.html
//! [`reload`]: struct.Reloadable.html#method.reload

extern crate notify;
extern crate libloading;

use std::path::{Path, PathBuf};
use std::time::Duration;
use std::sync::mpsc::{channel, Receiver};

use notify::{Watcher, RecommendedWatcher};
use libloading::Library;

#[cfg(unix)]
type Symbol<T> = libloading::os::unix::Symbol<T>;
#[cfg(windows)]
type Symbol<T> = libloading::os::windows::Symbol<T>;

struct AppSym {
    /// This needs to be present so that the library will be closed on drop
    _lib: Library,
    api: Symbol<*mut internals::ReloadApi>,
}

/// A `Reloadable` represents a handle to library that can be live reloaded.
///
/// Libraries that
pub struct Reloadable {
    path: PathBuf,
    sym: Option<AppSym>,
    state: Vec<u64>,
    _watcher: RecommendedWatcher,
    rx: Receiver<notify::DebouncedEvent>,
}

/// The errors that can occur while working with a `Reloadable` object.
#[derive(Debug)]
pub enum Error {
    /// An I/O error occurred while trying to load or reload the library. This
    /// can indicate that the file is missing, or that the library didn't have
    /// the expected `RELOAD_API` symbol.
    // @Diagnostics: Add an error type to distinguish this latter situation
    Io(std::io::Error),
    /// An error occurred while creating the filesystem watcher.
    Watch(notify::Error),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<notify::Error> for Error {
    fn from(err: notify::Error) -> Error {
        Error::Watch(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(fmt, "{:?}", self)
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match self {
            &Error::Io(ref err) => err.description(),
            &Error::Watch(ref err) => err.description(),
        }
    }
}

impl AppSym {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let library = Library::new(path.as_ref())?;
        let api = unsafe { library.get::<*mut internals::ReloadApi>(b"RELOAD_API")?.into_raw() };
        Ok(AppSym {
            _lib: library,
            api: api,
        })
    }
}

impl Reloadable {
    /// Create a new Reloadable library.
    ///
    /// This takes the path to a dynamic library containing a `RELOAD_API`
    /// symbol that exports the functions needed for live reloading. In order to
    /// define this symbol in your own reloadable libraries, see the
    /// [`live_reload!`][] macro. This will load the library and initialize a
    /// filesystem watcher pointing to the file in order to know when the
    /// library has changed.
    ///
    /// [`live_reload!`]: macro.live_reload.html
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let sym = AppSym::new(&path)?;
        let size = (unsafe { &**sym.api }.size)();
        let (tx, rx) = channel();
        let mut watcher = notify::watcher(tx, Duration::from_secs(1))?;
        let mut new_path = PathBuf::new();
        new_path.push(path);
        watcher.watch(
            new_path.parent().unwrap(),
            notify::RecursiveMode::NonRecursive,
        )?;
        let mut app = Reloadable {
            path: new_path.canonicalize()?,
            sym: Some(sym),
            state: Vec::new(),
            _watcher: watcher,
            rx: rx,
        };
        app.realloc_buffer(size);
        if let Some(AppSym { ref mut api, .. }) = app.sym {
            unsafe {
                ((***api).init)(Self::get_state_ptr(&mut app.state));
            }
        }
        Ok(app)
    }

    /// Reload the library if it has changed, otherwise do nothing.
    ///
    /// This will consult with the filesystem watcher, and if the library has
    /// been recreated or updated, it will call [`reload_now`][].
    ///
    /// [`reload_now`]: struct.Reloadable.html#method.reload_now
    pub fn reload(&mut self) -> Result<(), Error> {
        let mut should_reload = false;
        while let Ok(evt) = self.rx.try_recv() {
            use notify::DebouncedEvent::*;
            match evt {
                NoticeWrite(ref path) |
                Write(ref path) |
                Create(ref path) => {
                    if *path == self.path {
                        should_reload = true;
                    }
                }
                _ => {}
            }
        }

        if should_reload || self.sym.is_none() {
            self.reload_now()
        } else {
            Ok(())
        }
    }

    /// Immediately reload the library without checking whether it has changed.
    ///
    /// This first calls `unload` on the currently loaded library, then unloads
    /// the dynamic library. Next, it loads the new dynamic library, and calls
    /// `reload` on that. If the new library fails to load, this method will
    /// return an `Err` and the `Reloadable` will be left with no library
    /// loaded.
    ///
    /// [`update`]: struct.Reloadable.html#method.update
    pub fn reload_now(&mut self) -> Result<(), Error> {
        if let Some(AppSym { ref mut api, .. }) = self.sym {
            unsafe {
                ((***api).unload)(Self::get_state_ptr(&mut self.state));
            }
        }
        self.sym = None;
        let sym = AppSym::new(&self.path)?;
        // @Avoid reallocating if unnecessary
        self.realloc_buffer((unsafe { &**sym.api }.size)());
        unsafe {
            ((**sym.api).reload)(Self::get_state_ptr(&mut self.state));
        }
        self.sym = Some(sym);

        Ok(())
    }

    /// Call the update method on the library.
    ///
    /// If no library is currently loaded, this does nothing and returns
    /// [`ShouldQuit::No`](enum.ShouldQuit.html#).
    pub fn update(&mut self) -> ShouldQuit {
        if let Some(AppSym { ref mut api, .. }) = self.sym {
            unsafe { ((***api).update)(Self::get_state_ptr(&mut self.state)) }
        } else {
            ShouldQuit::No
        }
    }

    /// Reallocate the buffer used to store the `State`.
    fn realloc_buffer(&mut self, size: usize) {
        let alloc_size_u64s = (size + 7) / 8;
        self.state.resize(alloc_size_u64s, 0);
    }

    /// Get a void pointer to the `State` buffer.
    unsafe fn get_state_ptr(buffer: &mut Vec<u64>) -> *mut () {
        buffer.as_mut_ptr() as *mut ()
    }
}

impl Drop for Reloadable {
    fn drop(&mut self) {
        if let Some(AppSym { ref mut api, .. }) = self.sym {
            unsafe {
                ((***api).deinit)(Self::get_state_ptr(&mut self.state));
            }
        }
    }
}


/// Should the main program quit? More self-documenting than a boolean!
///
/// This type is returned by the [`update`][] method, since with a boolean it's
/// often unclear if `true` means "should continue" or "should quit".
///
/// [`update`]: struct.Reloadable.html#method.update
#[derive(Debug, PartialEq, Eq)]
pub enum ShouldQuit {
    /// The wrapped library thinks the main program should continue running.
    No = 0,
    /// The wrapped library thinks the main program should quit now.
    Yes = 1,
}

/// Exported for compilation reasons but not useful, only look if you're curious.
///
/// This module holds to the `ReloadApi` struct, which is what what is looked up
/// by the `Reloadable` in order to communicate with the reloadable library. It
/// needs to be exported in order to avoid forcing the type definition into the
/// pub symbols of the wrapped library. An instance of `ReloadApi` called
/// `RELOAD_API` is generated by the [`live_reload!`][] macro.
///
/// [`live_reload!`]: ../macro.live_reload.html
pub mod internals {
    /// Contains function pointers for all the parts of the reloadable object lifecycle.
    #[repr(C)]
    pub struct ReloadApi {
        /// Returns the size of the State struct so that the host can allocate
        /// space for it.
        pub size: fn() -> usize,
        /// Initializes the State struct when the program is first started.
        pub init: fn(*mut ()),
        /// Makes any necessary updates when the program is reloaded.
        ///
        /// This will probably be normally empty. If you changed the State
        /// struct since the last compile, then it won't necessarily be
        /// correctly initialized. For safety, you should make your State struct
        /// `#[repr(C)]` and only add members at the end.
        pub reload: fn(*mut ()),
        /// Update the
        pub update: fn(*mut ()) -> super::ShouldQuit,
        /// Prepare for the library to be unloaded before a new version loads.
        ///
        /// This will probably normally be empty except for short periods in
        /// development when you're making lots of live changes and need to do
        /// some kind of migration.
        pub unload: fn(*mut ()),
        /// Do final shutdowns before the program completely quits.
        pub deinit: fn(*mut ()),
    }
}

/// Declare the API functions for a live-reloadable library.
///
/// This generates wrappers around higher-level lifecycle functions, and then
/// exports them in a struct that the reloader can find.
///
/// You need to define a struct that represents the state of your program, and
/// methods for `init`, `reload`, `update`, `unload`, and `deinit`. `init` and
/// `deinit` are called at the very beginning and end of the program, and
/// `reload` and `unload` are called immediately after and before the library is
/// loaded/reloaded. `update` is called by the wrapping application as needed.
///
/// # Example
///
/// ```rust
/// # #[macro_use] extern crate live_reload;
/// # fn main() {}
/// # #[repr(C)] struct State {}
/// # fn my_init(_: &mut State) {}
/// # fn my_reload(_: &mut State) {}
/// # fn my_unload(_: &mut State) {}
/// # fn my_deinit(_: &mut State) {}
/// # use live_reload::ShouldQuit;
/// # fn my_update(_: &mut State) -> ShouldQuit { ShouldQuit::No }
/// live_reload! {
///     state: State;
///     init: my_init;
///     reload: my_reload;
///     update: my_update;
///     unload: my_unload;
///     deinit: my_deinit;
/// }
/// ```
#[macro_export]
macro_rules! live_reload {
    (state: $State:ty;
     init: $init:ident;
     reload: $reload:ident;
     update: $update:ident;
     unload: $unload:ident;
     deinit: $deinit:ident;) => {

        fn cast<'a>(raw_state: *mut ()) -> &'a mut $State {
            unsafe { &mut *(raw_state as *mut $State) }
        }

        fn init_wrapper(raw_state: *mut ()) {
            $init(cast(raw_state))
        }

        fn reload_wrapper(raw_state: *mut ()) {
            $reload(cast(raw_state))
        }

        fn update_wrapper(raw_state: *mut ()) -> ::live_reload::ShouldQuit {
            $update(cast(raw_state))
        }

        fn unload_wrapper(raw_state: *mut ()) {
            $unload(cast(raw_state))
        }

        fn deinit_wrapper(raw_state: *mut ()) {
            $deinit(cast(raw_state))
        }

        #[no_mangle]
        pub static RELOAD_API: ::live_reload::internals::ReloadApi = ::live_reload::internals::ReloadApi {
            size: ::std::mem::size_of::<$State>,
            init: init_wrapper,
            reload: reload_wrapper,
            update: update_wrapper,
            unload: unload_wrapper,
            deinit: deinit_wrapper,
        };
    }
}
