#[macro_use] extern crate error_chain;
extern crate notify;
extern crate libloading;

use std::path::{Path, PathBuf};
use std::os::raw::c_void;
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
    api: Symbol<*mut _ReloadApi>,
}

pub struct App {
    path: PathBuf,
    sym: Option<AppSym>,
    state: Vec<u64>,
    _watcher: RecommendedWatcher,
    rx: Receiver<notify::DebouncedEvent>,
}

error_chain! {
    foreign_links {
        Io(std::io::Error);
        Watch(notify::Error);
    }

    errors {
        FailedToReload {
            description("failed to reload")
            display("failed to reload")
        }
    }
}

impl AppSym {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let library = Library::new(path.as_ref())?;
        let api = unsafe { library.get::<*mut _ReloadApi>(b"RELOAD_API")?.into_raw() };
        Ok(AppSym {
            _lib: library,
            api: api,
        })
    }
}

impl App {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let sym = AppSym::new(&path)?;
        let size = (unsafe { &**sym.api }.size)();
        let (tx, rx) = channel();
        let mut watcher = notify::watcher(tx, Duration::from_secs(1))?;
        let mut new_path = PathBuf::new();
        new_path.push(path);
        watcher.watch(new_path.parent().unwrap(), notify::RecursiveMode::NonRecursive)?;
        let mut app = App {
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

    pub fn reload_now(&mut self) -> Result<()> {
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
            ((**sym.api).load)(Self::get_state_ptr(&mut self.state));
        }
        self.sym = Some(sym);

        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        let mut should_reload = false;
        while let Ok(evt) = self.rx.try_recv() {
            use notify::DebouncedEvent::*;
            match evt {
                NoticeWrite(ref path) | Write(ref path) | Create(ref path) => {
                    if *path == self.path {
                        should_reload = true;
                    }
                }
                _ => {}
            }
        }

        if should_reload {
            self.reload_now()
        } else {
            Ok(())
        }
    }

    pub fn update(&mut self) -> ShouldQuit {
        if let Some(AppSym { ref mut api, .. }) = self.sym {
            unsafe {
                ((***api).update)(Self::get_state_ptr(&mut self.state))
            }
        } else {
            ShouldQuit::No
        }
    }

    fn realloc_buffer(&mut self, size: usize) {
        let alloc_size_u64s = (size+7)/8;
        self.state.resize(alloc_size_u64s, 0);
    }

    unsafe fn get_state_ptr(buffer: &mut Vec<u64>) -> *mut c_void {
        buffer.as_mut_ptr() as *mut c_void
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(AppSym { ref mut api, .. }) = self.sym {
            unsafe {
                ((***api).deinit)(Self::get_state_ptr(&mut self.state));
            }
        }
    }
}


#[derive(Debug, PartialEq, Eq)]
pub enum ShouldQuit {
    No = 0,
    Yes = 1,
}

#[repr(C)]
pub struct _ReloadApi {
    pub size: fn() -> usize,
    pub init: fn(*mut c_void),
    pub load: fn(*mut c_void),
    pub update: fn(*mut c_void) -> ShouldQuit,
    pub unload: fn(*mut c_void),
    pub deinit: fn(*mut c_void),
}

#[macro_export]
macro_rules! reload_api {
    (state: $State:ty;
     init: $init:ident;
     load: $load:ident;
     update: $update:ident;
     unload: $unload:ident;
     deinit: $deinit:ident;) => {

        fn cast<'a>(raw_state: *mut ::std::os::raw::c_void) -> &'a mut $State {
            unsafe { &mut *(raw_state as *mut $State) }
        }

        fn init_wrapper(raw_state: *mut ::std::os::raw::c_void) {
            $init(cast(raw_state))
        }

        fn load_wrapper(raw_state: *mut ::std::os::raw::c_void) {
            $load(cast(raw_state))
        }

        fn update_wrapper(raw_state: *mut ::std::os::raw::c_void) -> ShouldQuit {
            $update(cast(raw_state))
        }

        fn unload_wrapper(raw_state: *mut ::std::os::raw::c_void) {
            $unload(cast(raw_state))
        }

        fn deinit_wrapper(raw_state: *mut ::std::os::raw::c_void) {
            $deinit(cast(raw_state))
        }

        #[no_mangle]
        pub static RELOAD_API: ::reload_api::_ReloadApi = ::reload_api::_ReloadApi {
            size: ::std::mem::size_of::<$State>,
            init: init_wrapper,
            load: load_wrapper,
            update: update_wrapper,
            unload: unload_wrapper,
            deinit: deinit_wrapper,
        };
    }
}
