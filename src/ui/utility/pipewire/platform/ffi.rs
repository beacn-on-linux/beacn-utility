//! FFI and safe wrappers around libpipewire, note that a lot of this may be unused, while working
//! on this the requirements changed rapidly. Be aware that this GENERALLY only covers stuff
//! I need here in the Beacn Utility; it's not a general-purpose wrapper.

#![allow(non_camel_case_types)]
#![allow(unused)]

use anyhow::{Result, anyhow};
use dlopen2::wrapper::{Container, WrapperApi};
use log::error;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::fd::{AsRawFd, RawFd};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub const PW_TYPE_INTERFACE_DEVICE: &str = "PipeWire:Interface:Device";
pub const PW_TYPE_INTERFACE_NODE: &str = "PipeWire:Interface:Node";
pub const PW_TYPE_INTERFACE_PORT: &str = "PipeWire:Interface:Port";
pub const PW_TYPE_INTERFACE_LINK: &str = "PipeWire:Interface:Link";

// These are vendored directly from pipewire, links point to the documentation.

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/utils/dict.h#L42
#[repr(C)]
pub struct spa_dict {
    pub flags: u32,
    pub n_items: u32,
    pub items: *const spa_dict_item,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/utils/dict.h#L34
#[repr(C)]
pub struct spa_dict_item {
    pub key: *const c_char,
    pub value: *const c_char,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/utils/list.h#L32
#[repr(C)]
struct spa_list {
    prev: *mut spa_list,
    next: *mut spa_list,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/pod/pod.h#L42
#[repr(C)]
pub struct spa_pod {
    pub size: u32,
    pub type_: u32,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/buffer/buffer.h#L94
#[repr(C)]
struct spa_buffer {
    n_metas: u32,
    n_datas: u32,
    metas: *mut c_void, // Needs spa_meta
    datas: *mut spa_data,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/buffer/buffer.h#L68
#[repr(C)]
struct spa_data {
    type_: u32,
    flags: u32,
    fd: i64,
    mapoffset: u32,
    maxsize: u32,
    data: *mut c_void,
    chunk: *mut spa_chunk,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/buffer/buffer.h?#L52
#[repr(C)]
struct spa_chunk {
    offset: u32,
    size: u32,
    stride: i32,
    flags: i32,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/utils/hook.h#L118
#[repr(C)]
struct spa_callbacks {
    funcs: *const c_void,
    data: *mut c_void,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/utils/hook.h#L416
#[repr(C)]
pub struct spa_hook {
    link: spa_list,
    cb: spa_callbacks,
    removed: Option<unsafe extern "C" fn(hook: *mut spa_hook)>,
    priv_: *mut c_void,
}

impl spa_hook {
    fn new() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

// https://man7.org/linux/man-pages/man3/timespec.3type.html
// A libc timespec, used by pw_loop_update_timer for the initial delay and repeat interval.
#[repr(C)]
struct timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

/// Decodes a spa_dict's contents into a HashMap, keeps things user-safe.
fn decode_props(dict: *const spa_dict) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if dict.is_null() {
        return out;
    }
    let dict = unsafe { &*dict };
    if dict.items.is_null() {
        return out;
    }
    for i in 0..dict.n_items as isize {
        let item = unsafe { &*dict.items.offset(i) };
        if item.key.is_null() || item.value.is_null() {
            continue;
        }
        let key = unsafe { CStr::from_ptr(item.key).to_string_lossy().into_owned() };
        let value = unsafe { CStr::from_ptr(item.value).to_string_lossy().into_owned() };
        out.insert(key, value);
    }
    out
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/core.h#L490
pub const PW_VERSION_REGISTRY_EVENTS: u32 = 0;

#[repr(C)]
struct pw_registry_events {
    version: u32,
    global: Option<
        unsafe extern "C" fn(
            data: *mut c_void,
            id: u32,
            permissions: u32,
            type_: *const c_char,
            version: u32,
            props: *const spa_dict,
        ),
    >,
    global_remove: Option<unsafe extern "C" fn(data: *mut c_void, id: u32)>,
}

// NOTE: We run version 0, so don't support bound_props
// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/core.h#L110
pub const PW_VERSION_CORE_EVENTS: u32 = 0;

#[repr(C)]
struct pw_core_events {
    version: u32,
    info: Option<unsafe extern "C" fn(data: *mut c_void, info: *const c_void)>,
    done: Option<unsafe extern "C" fn(data: *mut c_void, id: u32, seq: c_int)>,
    ping: Option<unsafe extern "C" fn(data: *mut c_void, id: u32, seq: c_int)>,
    error: Option<
        unsafe extern "C" fn(
            data: *mut c_void,
            id: u32,
            seq: c_int,
            res: c_int,
            message: *const c_char,
        ),
    >,
    remove_id: Option<unsafe extern "C" fn(data: *mut c_void, id: u32)>,
    bound_id: Option<unsafe extern "C" fn(data: *mut c_void, id: u32, global_id: u32)>,
    add_mem:
        Option<unsafe extern "C" fn(data: *mut c_void, id: u32, type_: u32, fd: c_int, flags: u32)>,
    remove_mem: Option<unsafe extern "C" fn(data: *mut c_void, id: u32)>,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/device.h#L38
#[repr(C)]
struct pw_device_info {
    id: u32,
    change_mask: u64,
    props: *mut spa_dict,
    params: *mut c_void, // Needs spa_param_info
    n_params: u32,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/device.h#L65
pub const PW_VERSION_DEVICE_EVENTS: u32 = 0;

#[repr(C)]
struct pw_device_events {
    version: u32,
    info: Option<unsafe extern "C" fn(data: *mut c_void, info: *const pw_device_info)>,
    param: Option<
        unsafe extern "C" fn(
            data: *mut c_void,
            seq: c_int,
            id: u32,
            index: u32,
            next: u32,
            param: *const spa_pod,
        ),
    >,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/node.h#L56
pub const PW_NODE_CHANGE_MASK_INPUT_PORTS: u64 = 1 << 0;
pub const PW_NODE_CHANGE_MASK_OUTPUT_PORTS: u64 = 1 << 1;
pub const PW_NODE_CHANGE_MASK_STATE: u64 = 1 << 2;
pub const PW_NODE_CHANGE_MASK_PROPS: u64 = 1 << 3;
pub const PW_NODE_CHANGE_MASK_PARAMS: u64 = 1 << 4;
pub const PW_NODE_CHANGE_MASK_ALL: u64 = (1 << 5) - 1;

#[repr(C)]
struct pw_node_info {
    id: u32,
    max_input_ports: u32,
    max_output_ports: u32,
    change_mask: u64,
    n_input_ports: u32,
    n_output_ports: u32,
    state: c_int,
    error: *const c_char,
    props: *mut spa_dict,
    params: *mut c_void, // Needs spa_param_info
    n_params: u32,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/node.h#L92
pub const PW_VERSION_NODE_EVENTS: u32 = 0;

#[repr(C)]
struct pw_node_events {
    version: u32,
    info: Option<unsafe extern "C" fn(data: *mut c_void, info: *const pw_node_info)>,
    param: Option<
        unsafe extern "C" fn(
            data: *mut c_void,
            seq: c_int,
            id: u32,
            index: u32,
            next: u32,
            param: *const spa_pod,
        ),
    >,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/port.h?#L49
pub const PW_PORT_CHANGE_MASK_PROPS: u64 = 1 << 0;
pub const PW_PORT_CHANGE_MASK_PARAMS: u64 = 1 << 1;
pub const PW_PORT_CHANGE_MASK_ALL: u64 = (1 << 2) - 1;

#[repr(C)]
struct pw_port_info {
    id: u32,
    direction: c_int,
    change_mask: u64,
    props: *mut spa_dict,
    params: *mut c_void, // Needs spa_param_info
    n_params: u32,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/port.h#L77
pub const PW_VERSION_PORT_EVENTS: u32 = 0;

#[repr(C)]
struct pw_port_events {
    version: u32,
    info: Option<unsafe extern "C" fn(data: *mut c_void, info: *const pw_port_info)>,
    param: Option<
        unsafe extern "C" fn(
            data: *mut c_void,
            seq: c_int,
            id: u32,
            index: u32,
            next: u32,
            param: *const spa_pod,
        ),
    >,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/link.h#L58
pub const PW_LINK_CHANGE_MASK_STATE: u64 = 1 << 0;
pub const PW_LINK_CHANGE_MASK_FORMAT: u64 = 1 << 1;
pub const PW_LINK_CHANGE_MASK_PROPS: u64 = 1 << 2;
pub const PW_LINK_CHANGE_MASK_ALL: u64 = (1 << 3) - 1;

#[repr(C)]
pub struct pw_link_info {
    pub id: u32,
    pub output_node_id: u32,
    pub output_port_id: u32,
    pub input_node_id: u32,
    pub input_port_id: u32,
    pub change_mask: u64,
    pub state: u32,
    pub error: *const c_char,
    pub format: *const spa_pod,
    pub props: *const spa_dict,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/link.h#L91
pub const PW_VERSION_LINK_EVENTS: u32 = 0;

#[repr(C)]
pub struct pw_link_events {
    pub version: u32,
    pub info: Option<unsafe extern "C" fn(data: *mut c_void, info: *const pw_link_info)>,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/stream.h#L417
// NOTE: Again, only VERSION 0 support only, so missing command and trigger_done
pub const PW_VERSION_STREAM_EVENTS: u32 = 0;

#[repr(C)]
struct pw_stream_events {
    version: u32,
    destroy: Option<unsafe extern "C" fn(data: *mut c_void)>,
    state_changed: Option<
        unsafe extern "C" fn(data: *mut c_void, old: c_int, state: c_int, error: *const c_char),
    >,
    control_info: Option<unsafe extern "C" fn(data: *mut c_void, id: u32, control: *const c_void)>,
    io_changed:
        Option<unsafe extern "C" fn(data: *mut c_void, id: u32, area: *mut c_void, size: u32)>,
    param_changed: Option<unsafe extern "C" fn(data: *mut c_void, id: u32, param: *const c_void)>,
    add_buffer: Option<unsafe extern "C" fn(data: *mut c_void, buffer: *mut pw_buffer)>,
    remove_buffer: Option<unsafe extern "C" fn(data: *mut c_void, buffer: *mut pw_buffer)>,
    process: Option<unsafe extern "C" fn(data: *mut c_void)>,
    drained: Option<unsafe extern "C" fn(data: *mut c_void)>,
}

// https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/src/pipewire/stream.h#L256
#[repr(C)]
struct pw_buffer {
    buffer: *mut spa_buffer,
    user_data: *mut c_void,
    size: u64,
    requested: u64,
    time: u64,
}

// Random shit that's kinda useful :D
pub const PW_ID_ANY: u32 = 0xffffffff;
pub const PW_ID_CORE: u32 = 0;

// Port directions
pub const PW_DIRECTION_INPUT: c_int = 0;
pub const PW_DIRECTION_OUTPUT: c_int = 1;

// Stream States
pub const PW_STREAM_STATE_ERROR: c_int = -1;
pub const PW_STREAM_STATE_UNCONNECTED: c_int = 0;
pub const PW_STREAM_STATE_CONNECTING: c_int = 1;
pub const PW_STREAM_STATE_PAUSED: c_int = 2;
pub const PW_STREAM_STATE_STREAMING: c_int = 3;

// Stream Flags
pub mod stream_flags {
    use std::os::raw::c_int;
    pub const NONE: c_int = 0;
    pub const AUTOCONNECT: c_int = 1 << 0;
    pub const INACTIVE: c_int = 1 << 1;
    pub const MAP_BUFFERS: c_int = 1 << 2;
    pub const DRIVER: c_int = 1 << 3;
    pub const RT_PROCESS: c_int = 1 << 4;
    pub const NO_CONVERT: c_int = 1 << 5;
    pub const EXCLUSIVE: c_int = 1 << 6;
    pub const DONT_RECONNECT: c_int = 1 << 7;
    pub const ALLOC_BUFFERS: c_int = 1 << 8;
    pub const TRIGGER: c_int = 1 << 9;
}

// Ok, this is the dlopen FFI wrapper
#[derive(WrapperApi)]
struct PwApi {
    pw_init: unsafe extern "C" fn(argc: *mut c_int, argv: *mut *mut *mut c_char),
    pw_deinit: unsafe extern "C" fn(),
    pw_get_library_version: unsafe extern "C" fn() -> *const c_char,

    pw_main_loop_new: unsafe extern "C" fn(props: *const spa_dict) -> *mut c_void,
    pw_main_loop_get_loop: unsafe extern "C" fn(loop_: *mut c_void) -> *mut c_void,
    pw_main_loop_run: unsafe extern "C" fn(loop_: *mut c_void) -> c_int,
    pw_main_loop_quit: unsafe extern "C" fn(loop_: *mut c_void) -> c_int,
    pw_main_loop_destroy: unsafe extern "C" fn(loop_: *mut c_void),

    pw_loop_new: unsafe extern "C" fn(props: *const spa_dict) -> *mut c_void,
    pw_loop_destroy: unsafe extern "C" fn(loop_: *mut c_void),
    pw_loop_get_fd: unsafe extern "C" fn(loop_: *mut c_void) -> c_int,
    pw_loop_enter: unsafe extern "C" fn(loop_: *mut c_void),
    pw_loop_iterate: unsafe extern "C" fn(loop_: *mut c_void, timeout_ms: c_int) -> c_int,
    pw_loop_leave: unsafe extern "C" fn(loop_: *mut c_void),

    pw_loop_add_timer: unsafe extern "C" fn(
        loop_: *mut c_void,
        func: unsafe extern "C" fn(data: *mut c_void, expirations: u64),
        data: *mut c_void,
    ) -> *mut c_void,
    pw_loop_update_timer: unsafe extern "C" fn(
        loop_: *mut c_void,
        source: *mut c_void,
        value: *const timespec,
        interval: *const timespec,
        absolute: c_int,
    ) -> c_int,

    pw_context_new: unsafe extern "C" fn(
        main_loop: *mut c_void,
        props: *mut c_void,
        user_data_size: usize,
    ) -> *mut c_void,
    pw_context_destroy: unsafe extern "C" fn(context: *mut c_void),
    pw_context_connect: unsafe extern "C" fn(
        context: *mut c_void,
        properties: *mut c_void,
        user_data_size: usize,
    ) -> *mut c_void,

    pw_core_disconnect: unsafe extern "C" fn(core: *mut c_void) -> c_int,
    pw_core_add_listener: unsafe extern "C" fn(
        core: *mut c_void,
        listener: *mut spa_hook,
        events: *const pw_core_events,
        data: *mut c_void,
    ) -> c_int,
    pw_core_sync: unsafe extern "C" fn(core: *mut c_void, id: u32, seq: c_int) -> c_int,
    pw_core_get_registry:
        unsafe extern "C" fn(core: *mut c_void, version: u32, user_data_size: usize) -> *mut c_void,
    pw_core_create_object: unsafe extern "C" fn(
        core: *mut c_void,
        factory_name: *const c_char,
        type_: *const c_char,
        version: u32,
        props: *const spa_dict,
        user_data_size: usize,
    ) -> *mut c_void,

    pw_registry_add_listener: unsafe extern "C" fn(
        registry: *mut c_void,
        listener: *mut spa_hook,
        events: *const pw_registry_events,
        data: *mut c_void,
    ) -> c_int,
    pw_registry_bind: unsafe extern "C" fn(
        registry: *mut c_void,
        id: u32,
        type_: *const c_char,
        version: u32,
        user_data_size: usize,
    ) -> *mut c_void,

    pw_proxy_destroy: unsafe extern "C" fn(proxy: *mut c_void),

    pw_device_add_listener: unsafe extern "C" fn(
        device: *mut c_void,
        listener: *mut spa_hook,
        events: *const pw_device_events,
        data: *mut c_void,
    ) -> c_int,
    pw_node_add_listener: unsafe extern "C" fn(
        node: *mut c_void,
        listener: *mut spa_hook,
        events: *const pw_node_events,
        data: *mut c_void,
    ) -> c_int,
    pw_port_add_listener: unsafe extern "C" fn(
        port: *mut c_void,
        listener: *mut spa_hook,
        events: *const pw_port_events,
        data: *mut c_void,
    ) -> c_int,
    pw_link_add_listener: unsafe extern "C" fn(
        link: *mut c_void,
        listener: *mut spa_hook,
        events: *const pw_link_events,
        data: *mut c_void,
    ) -> c_int,

    pw_properties_new: unsafe extern "C" fn(key: *const c_char) -> *mut c_void,
    pw_properties_set: unsafe extern "C" fn(
        properties: *mut c_void,
        key: *const c_char,
        value: *const c_char,
    ) -> c_int,
    pw_properties_free: unsafe extern "C" fn(properties: *mut c_void),

    pw_stream_new: unsafe extern "C" fn(
        core: *mut c_void,
        name: *const c_char,
        props: *mut c_void,
    ) -> *mut c_void,
    pw_stream_add_listener: unsafe extern "C" fn(
        stream: *mut c_void,
        listener: *mut spa_hook,
        events: *const pw_stream_events,
        data: *mut c_void,
    ),
    pw_stream_connect: unsafe extern "C" fn(
        stream: *mut c_void,
        direction: c_int,
        target_id: u32,
        flags: c_int,
        params: *const *const c_void,
        n_params: u32,
    ) -> c_int,
    pw_stream_get_node_id: unsafe extern "C" fn(stream: *mut c_void) -> u32,
    pw_stream_dequeue_buffer: unsafe extern "C" fn(stream: *mut c_void) -> *mut pw_buffer,
    pw_stream_queue_buffer:
        unsafe extern "C" fn(stream: *mut c_void, buffer: *mut pw_buffer) -> c_int,
    pw_stream_destroy: unsafe extern "C" fn(stream: *mut c_void),
}

fn load() -> Result<Container<PwApi>, dlopen2::Error> {
    unsafe {
        Container::load("libpipewire-0.3.so.0").or_else(|_| Container::load("libpipewire-0.3.so"))
    }
}

// Listener Storage
struct RegisteredListener {
    _hook: Box<spa_hook>,
    _keepalive: Box<dyn std::any::Any>,
}

// This is mostly internal, although it could be exposed at some point. It's primarily a
// holding struct for an Arc<AtomicBool>> which, if true, will stop the main loop.
struct StopPollCtx {
    pw: PipeWire,
    main_loop_ptr: *mut c_void,
    stop: Arc<AtomicBool>,
}

extern "C" fn stop_poll_trampoline(data: *mut c_void, _expirations: u64) {
    let ctx = unsafe { &*(data as *const StopPollCtx) };
    if ctx.stop.load(std::sync::atomic::Ordering::Relaxed) {
        unsafe { (ctx.pw.api.pw_main_loop_quit)(ctx.main_loop_ptr) };
    }
}

// Ok, this is the main PipeWire FFI wrapper, there are two loop variants in here, the async and
// the blocking one.. Ideally I can fix them in a way I don't need to duplicate.
#[derive(Clone)]
pub struct PipeWire {
    api: Arc<Container<PwApi>>,
}
unsafe impl Send for PipeWire {}
unsafe impl Sync for PipeWire {}

impl PipeWire {
    pub fn load() -> Result<Self> {
        Ok(Self {
            api: Arc::new(load()?),
        })
    }
    pub fn init(&self) {
        let mut argc: c_int = 0;
        unsafe { (self.api.pw_init)(&mut argc, std::ptr::null_mut()) };
    }
    pub fn deinit(&self) {
        unsafe { (self.api.pw_deinit)() };
    }

    pub fn library_version(&self) -> String {
        unsafe {
            let ptr = (self.api.pw_get_library_version)();
            if ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }

    pub fn main_loop_new(&self) -> Result<PwMainLoop> {
        let ptr = unsafe { (self.api.pw_main_loop_new)(std::ptr::null()) };
        if ptr.is_null() {
            return Err(anyhow!("pw_main_loop_new failed"));
        }
        Ok(PwMainLoop {
            pw: self.clone(),
            ptr,
            keepalive: Vec::new(),
        })
    }

    pub fn async_loop_new(&self) -> Result<PwAsyncLoop> {
        let ptr = unsafe { (self.api.pw_loop_new)(std::ptr::null()) };
        if ptr.is_null() {
            return Err(anyhow!("pw_loop_new failed"));
        }
        Ok(PwAsyncLoop {
            pw: self.clone(),
            ptr,
        })
    }

    pub fn context_new(&self, loop_: &impl PwLoop) -> Result<PwContext> {
        let ptr = unsafe { (self.api.pw_context_new)(loop_.get_ptr(), std::ptr::null_mut(), 0) };
        if ptr.is_null() {
            return Err(anyhow!("pw_context_new failed"));
        }
        Ok(PwContext {
            pw: self.clone(),
            ptr,
        })
    }
}

// Loop Variants, either blocking MainLoop or enterable AsyncLoop
pub trait PwLoop {
    fn get_ptr(&self) -> *mut c_void;
}

pub struct PwMainLoop {
    pw: PipeWire,
    ptr: *mut c_void,

    // Keeps stuff alive as long as the mainloop exists
    keepalive: Vec<Box<dyn std::any::Any>>,
}
unsafe impl Send for PwMainLoop {}
impl PwMainLoop {
    pub fn run(&self) {
        unsafe { (self.pw.api.pw_main_loop_run)(self.ptr) };
    }
    pub fn quit(&self) {
        unsafe { (self.pw.api.pw_main_loop_quit)(self.ptr) };
    }

    // This is a helper hack, I tend to use AtomicBools to signal when I want shit to stop, so
    // this creates a repeating timer in the pipewire loop to see if stop has become true, and
    // if so, quits the loop.
    pub fn quit_when(&mut self, stop: Arc<AtomicBool>, poll_every_ms: u32) -> Result<()> {
        let loop_ptr = self.get_ptr(); // pw_main_loop_get_loop

        let ctx = Box::new(StopPollCtx {
            pw: self.pw.clone(),
            main_loop_ptr: self.ptr,
            stop,
        });
        let ctx_ptr = ctx.as_ref() as *const StopPollCtx as *mut c_void;

        let source =
            unsafe { (self.pw.api.pw_loop_add_timer)(loop_ptr, stop_poll_trampoline, ctx_ptr) };
        if source.is_null() {
            return Err(anyhow!("pw_loop_add_timer failed"));
        }

        let interval = timespec {
            tv_sec: (poll_every_ms / 1000) as i64,
            tv_nsec: ((poll_every_ms % 1000) as i64) * 1_000_000,
        };
        // value == interval here: first fire after one interval, then repeat at
        // that same interval. absolute = 0 (false), i.e. relative timing.
        let res = unsafe {
            (self.pw.api.pw_loop_update_timer)(loop_ptr, source, &interval, &interval, 0)
        };
        if res < 0 {
            return Err(anyhow!("pw_loop_update_timer failed: {res}"));
        }

        self.keepalive.push(ctx);
        Ok(())
    }
}
impl Drop for PwMainLoop {
    fn drop(&mut self) {
        unsafe { (self.pw.api.pw_main_loop_destroy)(self.ptr) };
    }
}
impl PwLoop for PwMainLoop {
    fn get_ptr(&self) -> *mut c_void {
        unsafe { (self.pw.api.pw_main_loop_get_loop)(self.ptr) }
    }
}

pub struct PwAsyncLoop {
    pw: PipeWire,
    ptr: *mut c_void,
}
unsafe impl Send for PwAsyncLoop {}
impl PwAsyncLoop {
    pub fn get_fd(&self) -> c_int {
        unsafe { (self.pw.api.pw_loop_get_fd)(self.ptr) }
    }
    pub fn enter(&self) {
        unsafe { (self.pw.api.pw_loop_enter)(self.ptr) };
    }
    pub fn leave(&self) {
        unsafe { (self.pw.api.pw_loop_leave)(self.ptr) };
    }
    pub fn iterate(&self, timeout_ms: c_int) -> c_int {
        unsafe { (self.pw.api.pw_loop_iterate)(self.ptr, timeout_ms) }
    }
}
impl Drop for PwAsyncLoop {
    fn drop(&mut self) {
        unsafe { (self.pw.api.pw_loop_destroy)(self.ptr) };
    }
}
impl PwLoop for PwAsyncLoop {
    fn get_ptr(&self) -> *mut c_void {
        self.ptr
    }
}

// Context Wrapper
pub struct PwContext {
    pw: PipeWire,
    ptr: *mut c_void,
}
unsafe impl Send for PwContext {}
impl PwContext {
    pub fn connect(&self) -> Result<PwCore> {
        let ptr = unsafe { (self.pw.api.pw_context_connect)(self.ptr, std::ptr::null_mut(), 0) };
        if ptr.is_null() {
            return Err(anyhow!("pw_context_connect failed -- is PipeWire running?"));
        }
        Ok(PwCore {
            pw: self.pw.clone(),
            ptr,
            listeners: Vec::new(),
        })
    }
}
impl Drop for PwContext {
    fn drop(&mut self) {
        unsafe { (self.pw.api.pw_context_destroy)(self.ptr) };
    }
}

// Core Wrapper. We should probably have builders for the listeners, but for my use cases, this
// is fine for now.
pub struct PwCore {
    pw: PipeWire,
    ptr: *mut c_void,
    listeners: Vec<RegisteredListener>,
}
unsafe impl Send for PwCore {}

impl PwCore {
    pub fn add_done_listener<F>(&mut self, on_done: F) -> Result<()>
    where
        F: FnMut(u32, i32) + 'static,
    {
        struct Ctx<F> {
            f: F,
        }
        extern "C" fn trampoline<F: FnMut(u32, i32)>(data: *mut c_void, id: u32, seq: c_int) {
            let ctx = unsafe { &mut *(data as *mut Ctx<F>) };
            (ctx.f)(id, seq);
        }

        let mut ctx = Box::new(Ctx { f: on_done });
        let ctx_ptr = ctx.as_mut() as *mut Ctx<F> as *mut c_void;

        let mut events = Box::new(pw_core_events {
            version: PW_VERSION_CORE_EVENTS,
            info: None,
            done: Some(trampoline::<F>),
            ping: None,
            error: None,
            remove_id: None,
            bound_id: None,
            add_mem: None,
            remove_mem: None,
        });
        let events_ptr = events.as_mut() as *mut pw_core_events;

        let mut hook = Box::new(spa_hook::new());
        let res = unsafe {
            (self.pw.api.pw_core_add_listener)(self.ptr, hook.as_mut(), events_ptr, ctx_ptr)
        };
        if res < 0 {
            return Err(anyhow!("pw_core_add_listener failed: {res}"));
        }
        self.listeners.push(RegisteredListener {
            _hook: hook,
            _keepalive: Box::new((events, ctx)),
        });
        Ok(())
    }

    pub fn add_error_listener<F>(&mut self, on_error: F) -> Result<()>
    where
        F: FnMut(u32, i32, i32, String) + 'static,
    {
        struct Ctx<F> {
            f: F,
        }
        extern "C" fn trampoline<F: FnMut(u32, i32, i32, String)>(
            data: *mut c_void,
            id: u32,
            seq: c_int,
            res: c_int,
            message: *const c_char,
        ) {
            let ctx = unsafe { &mut *(data as *mut Ctx<F>) };
            let msg = if message.is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(message) }
                    .to_string_lossy()
                    .into_owned()
            };
            (ctx.f)(id, seq, res, msg);
        }

        let mut ctx = Box::new(Ctx { f: on_error });
        let ctx_ptr = ctx.as_mut() as *mut Ctx<F> as *mut c_void;

        let mut events = Box::new(pw_core_events {
            version: PW_VERSION_CORE_EVENTS,
            info: None,
            done: None,
            ping: None,
            error: Some(trampoline::<F>),
            remove_id: None,
            bound_id: None,
            add_mem: None,
            remove_mem: None,
        });
        let events_ptr = events.as_mut() as *mut pw_core_events;

        let mut hook = Box::new(spa_hook::new());
        let res = unsafe {
            (self.pw.api.pw_core_add_listener)(self.ptr, hook.as_mut(), events_ptr, ctx_ptr)
        };
        if res < 0 {
            return Err(anyhow!("pw_core_add_listener (error) failed: {res}"));
        }
        self.listeners.push(RegisteredListener {
            _hook: hook,
            _keepalive: Box::new((events, ctx)),
        });
        Ok(())
    }

    pub fn add_bound_id_listener<F>(&mut self, on_bound: F) -> Result<()>
    where
        F: FnMut(u32, u32) + 'static,
    {
        struct Ctx<F> {
            f: F,
        }
        extern "C" fn trampoline<F: FnMut(u32, u32)>(data: *mut c_void, id: u32, global_id: u32) {
            let ctx = unsafe { &mut *(data as *mut Ctx<F>) };
            (ctx.f)(id, global_id);
        }

        let mut ctx = Box::new(Ctx { f: on_bound });
        let ctx_ptr = ctx.as_mut() as *mut Ctx<F> as *mut c_void;

        let mut events = Box::new(pw_core_events {
            version: PW_VERSION_CORE_EVENTS,
            info: None,
            done: None,
            ping: None,
            error: None,
            remove_id: None,
            bound_id: Some(trampoline::<F>),
            add_mem: None,
            remove_mem: None,
        });
        let events_ptr = events.as_mut() as *mut pw_core_events;

        let mut hook = Box::new(spa_hook::new());
        let res = unsafe {
            (self.pw.api.pw_core_add_listener)(self.ptr, hook.as_mut(), events_ptr, ctx_ptr)
        };
        if res < 0 {
            return Err(anyhow!("pw_core_add_listener (bound_id) failed: {res}"));
        }
        self.listeners.push(RegisteredListener {
            _hook: hook,
            _keepalive: Box::new((events, ctx)),
        });
        Ok(())
    }

    pub fn sync(&self, seq: i32) -> i32 {
        unsafe { (self.pw.api.pw_core_sync)(self.ptr, PW_ID_CORE, seq) }
    }

    pub fn get_registry(&self) -> Result<PwRegistry> {
        let ptr = unsafe { (self.pw.api.pw_core_get_registry)(self.ptr, 3, 0) };
        if ptr.is_null() {
            return Err(anyhow!("pw_core_get_registry failed"));
        }
        Ok(PwRegistry {
            pw: self.pw.clone(),
            ptr,
            listeners: Vec::new(),
        })
    }

    pub fn create_object(
        &self,
        factory_name: &str,
        type_: &str,
        version: u32,
        props: &PwProperties,
    ) -> Result<PwProxy> {
        let factory_name_c = CString::new(factory_name)?;
        let type_c = CString::new(type_)?;
        let dict_ptr = props.ptr as *const spa_dict;
        let ptr = unsafe {
            (self.pw.api.pw_core_create_object)(
                self.ptr,
                factory_name_c.as_ptr(),
                type_c.as_ptr(),
                version,
                dict_ptr,
                0,
            )
        };
        if ptr.is_null() {
            return Err(anyhow!("pw_core_create_object({factory_name}) failed"));
        }
        Ok(PwProxy {
            pw: self.pw.clone(),
            ptr,
        })
    }
}

impl Drop for PwCore {
    fn drop(&mut self) {
        let res = unsafe { (self.pw.api.pw_core_disconnect)(self.ptr) };
        if res < 0 {
            error!("pw_core_disconnect failed: {res}");
        }
    }
}

// Registry Wrapper
pub struct PwRegistry {
    pw: PipeWire,
    ptr: *mut c_void,
    listeners: Vec<RegisteredListener>,
}
unsafe impl Send for PwRegistry {}

impl PwRegistry {
    pub fn add_global_listener<F>(&mut self, on_global: F) -> Result<()>
    where
        F: FnMut(u32, u32, &str, u32, HashMap<String, String>) + 'static,
    {
        struct Ctx<F> {
            f: F,
        }
        extern "C" fn trampoline<F: FnMut(u32, u32, &str, u32, HashMap<String, String>)>(
            data: *mut c_void,
            id: u32,
            permissions: u32,
            type_: *const c_char,
            version: u32,
            props: *const spa_dict,
        ) {
            let ctx = unsafe { &mut *(data as *mut Ctx<F>) };
            let type_str = unsafe { CStr::from_ptr(type_) }.to_string_lossy();
            let decoded = decode_props(props);
            (ctx.f)(id, permissions, &type_str, version, decoded);
        }

        let mut ctx = Box::new(Ctx { f: on_global });
        let ctx_ptr = ctx.as_mut() as *mut Ctx<F> as *mut c_void;

        let mut events = Box::new(pw_registry_events {
            version: PW_VERSION_REGISTRY_EVENTS,
            global: Some(trampoline::<F>),
            global_remove: None,
        });
        let events_ptr = events.as_mut() as *mut pw_registry_events;

        let mut hook = Box::new(spa_hook::new());
        let res = unsafe {
            (self.pw.api.pw_registry_add_listener)(self.ptr, hook.as_mut(), events_ptr, ctx_ptr)
        };
        if res < 0 {
            return Err(anyhow!("pw_registry_add_listener failed: {res}"));
        }
        self.listeners.push(RegisteredListener {
            _hook: hook,
            _keepalive: Box::new((events, ctx)),
        });
        Ok(())
    }

    pub fn bind(&self, id: u32, type_: &str, version: u32) -> Result<PwProxy> {
        let type_c = CString::new(type_)?;
        let ptr =
            unsafe { (self.pw.api.pw_registry_bind)(self.ptr, id, type_c.as_ptr(), version, 0) };
        if ptr.is_null() {
            return Err(anyhow!("pw_registry_bind({id}) failed"));
        }
        Ok(PwProxy {
            pw: self.pw.clone(),
            ptr,
        })
    }
}

impl Drop for PwRegistry {
    fn drop(&mut self) {
        unsafe { (self.pw.api.pw_proxy_destroy)(self.ptr) };
    }
}

// Generic Proxy Type
pub struct PwProxy {
    pw: PipeWire,
    ptr: *mut c_void,
}
unsafe impl Send for PwProxy {}
impl Drop for PwProxy {
    fn drop(&mut self) {
        unsafe { (self.pw.api.pw_proxy_destroy)(self.ptr) };
    }
}

// Helper types, get populated from their respective pipewire structs.
// Note, these aren't complete, just contain stuff I need :)
pub struct DeviceInfo {
    pub id: u32,
    pub props: HashMap<String, String>,
}
pub struct NodeInfo {
    pub id: u32,
    pub change_mask: u64,
    pub n_input_ports: u32,
    pub n_output_ports: u32,
    pub props: HashMap<String, String>,
}
#[derive(Debug)]
pub struct PortInfo {
    pub id: u32,
    pub direction_is_output: bool,
    pub props: HashMap<String, String>,
}
#[derive(Debug)]
pub struct LinkInfo {
    pub id: u32,
    pub state: u32,
    pub error: Option<String>,
}

// This is laziness, for Devices, Nodes, Ports and Links we only care about info callbacks, so
// wrap them all up in a nice bow :D
macro_rules! typed_proxy {
    (
        $name:ident,
        $add_listener_fn:ident,
        $events_ty:ident,
        $version_const:expr,
        $raw_info:ty,
        $info_out:ty,
        $decode:expr,
        $make_events:expr
    ) => {
        pub struct $name {
            proxy: PwProxy,
            listeners: Vec<RegisteredListener>,
        }

        impl $name {
            pub fn from_proxy(proxy: PwProxy) -> Self {
                Self {
                    proxy,
                    listeners: Vec::new(),
                }
            }

            pub fn add_info_listener<F>(&mut self, on_info: F) -> Result<()>
            where
                F: FnMut($info_out) + 'static,
            {
                struct Ctx<F> {
                    f: F,
                }

                extern "C" fn trampoline<F: FnMut($info_out)>(
                    data: *mut c_void,
                    info: *const $raw_info,
                ) {
                    if info.is_null() {
                        return;
                    }

                    let ctx = unsafe { &mut *(data as *mut Ctx<F>) };
                    let decoded = unsafe { $decode(&*info) };
                    (ctx.f)(decoded);
                }

                let mut ctx = Box::new(Ctx { f: on_info });
                let ctx_ptr = ctx.as_mut() as *mut Ctx<F> as *mut c_void;

                let mut events = Box::new($make_events(trampoline::<F>));

                let events_ptr = events.as_mut() as *mut $events_ty;

                let mut hook = Box::new(spa_hook::new());

                let res = unsafe {
                    (self.proxy.pw.api.$add_listener_fn)(
                        self.proxy.ptr,
                        hook.as_mut(),
                        events_ptr,
                        ctx_ptr,
                    )
                };

                if res < 0 {
                    return Err(anyhow!(
                        concat!(stringify!($add_listener_fn), " failed: {res}"),
                        res = res
                    ));
                }

                self.listeners.push(RegisteredListener {
                    _hook: hook,
                    _keepalive: Box::new((events, ctx)),
                });

                Ok(())
            }
        }
    };
}

typed_proxy!(
    PwDeviceProxy,
    pw_device_add_listener,
    pw_device_events,
    PW_VERSION_DEVICE_EVENTS,
    pw_device_info,
    DeviceInfo,
    |info: &pw_device_info| DeviceInfo {
        id: info.id,
        props: decode_props(info.props)
    },
    |info| pw_device_events {
        version: PW_VERSION_DEVICE_EVENTS,
        info: Some(info),
        param: None,
    }
);

typed_proxy!(
    PwNodeProxy,
    pw_node_add_listener,
    pw_node_events,
    PW_VERSION_NODE_EVENTS,
    pw_node_info,
    NodeInfo,
    |info: &pw_node_info| NodeInfo {
        id: info.id,
        change_mask: info.change_mask,
        n_input_ports: info.n_input_ports,
        n_output_ports: info.n_output_ports,
        props: decode_props(info.props)
    },
    |info| pw_node_events {
        version: PW_VERSION_DEVICE_EVENTS,
        info: Some(info),
        param: None,
    }
);
typed_proxy!(
    PwPortProxy,
    pw_port_add_listener,
    pw_port_events,
    PW_VERSION_PORT_EVENTS,
    pw_port_info,
    PortInfo,
    |info: &pw_port_info| PortInfo {
        id: info.id,
        direction_is_output: info.direction == PW_DIRECTION_OUTPUT,
        props: decode_props(info.props),
    },
    |info| pw_port_events {
        version: PW_VERSION_DEVICE_EVENTS,
        info: Some(info),
        param: None,
    }
);
typed_proxy!(
    PwLinkProxy,
    pw_link_add_listener,
    pw_link_events,
    PW_VERSION_LINK_EVENTS,
    pw_link_info,
    LinkInfo,
    |info: &pw_link_info| {
        let error = if info.error.is_null() {
            None
        } else {
            Some(CStr::from_ptr(info.error).to_string_lossy().into_owned())
        };
        LinkInfo {
            id: info.id,
            state: info.state,
            error,
        }
    },
    |info| pw_link_events {
        version: PW_VERSION_LINK_EVENTS,
        info: Some(info),
    }
);

// Properties Wrapper
pub struct PwProperties {
    pw: PipeWire,
    ptr: *mut c_void,
}
unsafe impl Send for PwProperties {}

impl PwProperties {
    pub fn new(pw: &PipeWire) -> Result<Self> {
        let ptr = unsafe { (pw.api.pw_properties_new)(std::ptr::null()) };
        if ptr.is_null() {
            return Err(anyhow!("pw_properties_new failed"));
        }
        Ok(Self {
            pw: pw.clone(),
            ptr,
        })
    }

    pub fn from_map(pw: &PipeWire, map: &HashMap<String, String>) -> Result<Self> {
        let mut props = Self::new(pw)?;
        for (k, v) in map {
            props.set(k, v)?;
        }
        Ok(props)
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let key_c = CString::new(key)?;
        let value_c = CString::new(value)?;
        let res =
            unsafe { (self.pw.api.pw_properties_set)(self.ptr, key_c.as_ptr(), value_c.as_ptr()) };
        if res < 0 {
            return Err(anyhow!("pw_properties_set({key}) failed: {res}"));
        }
        Ok(())
    }

    pub fn to_map(&self) -> HashMap<String, String> {
        decode_props(self.ptr as *const spa_dict)
    }
}

impl Drop for PwProperties {
    fn drop(&mut self) {
        unsafe { (self.pw.api.pw_properties_free)(self.ptr) };
    }
}

// Stream Handling
pub struct StreamBuffer {
    pw: PipeWire,
    stream_ptr: *mut c_void,
    raw: *mut pw_buffer,
}
impl StreamBuffer {
    pub fn channel_count(&self) -> usize {
        unsafe { (*(*self.raw).buffer).n_datas as usize }
    }
    pub fn channel_samples(&self, channel: usize) -> &[f32] {
        unsafe {
            let spa_buf = &*(*self.raw).buffer;
            if channel >= spa_buf.n_datas as usize {
                return &[];
            }
            let d = &*spa_buf.datas.add(channel);
            if d.data.is_null() || d.chunk.is_null() {
                return &[];
            }
            let chunk = &*d.chunk;
            let n = (chunk.size as usize) / size_of::<f32>();
            let ptr = (d.data as *const u8).add(chunk.offset as usize) as *const f32;
            std::slice::from_raw_parts(ptr, n)
        }
    }
    pub fn channel_samples_mut(&mut self, channel: usize) -> &mut [f32] {
        unsafe {
            let spa_buf = &*(*self.raw).buffer;
            if channel >= spa_buf.n_datas as usize {
                return &mut [];
            }
            let d = &*spa_buf.datas.add(channel);
            if d.data.is_null() || d.chunk.is_null() {
                return &mut [];
            }
            let chunk = &*d.chunk;
            let n = (chunk.size as usize) / size_of::<f32>();
            let ptr = (d.data as *const u8).add(chunk.offset as usize) as *mut f32;
            std::slice::from_raw_parts_mut(ptr, n)
        }
    }
}
impl Drop for StreamBuffer {
    fn drop(&mut self) {
        let res = unsafe { (self.pw.api.pw_stream_queue_buffer)(self.stream_ptr, self.raw) };
        if res < 0 {
            error!("pw_stream_queue_buffer failed: {res}");
        }
    }
}

type StreamStateChangeCallback = Option<Box<dyn FnMut(i32, i32, Option<String>)>>;
#[derive(Default)]
// We should probably do this for other types, this code came later :D
pub struct StreamCallbacks {
    pub process: Option<Box<dyn FnMut(StreamBuffer)>>,
    pub state_changed: StreamStateChangeCallback,
    pub param_changed: Option<Box<dyn FnMut(u32)>>,
}

struct StreamCtx {
    pw: PipeWire,
    stream_ptr: *mut c_void,
    callbacks: StreamCallbacks,
}

extern "C" fn stream_process_trampoline(data: *mut c_void) {
    let ctx = unsafe { &mut *(data as *mut StreamCtx) };
    let raw = unsafe { (ctx.pw.api.pw_stream_dequeue_buffer)(ctx.stream_ptr) };
    if raw.is_null() {
        return;
    }
    if let Some(cb) = ctx.callbacks.process.as_mut() {
        let buf = StreamBuffer {
            pw: ctx.pw.clone(),
            stream_ptr: ctx.stream_ptr,
            raw,
        };
        cb(buf);
    } else {
        unsafe { (ctx.pw.api.pw_stream_queue_buffer)(ctx.stream_ptr, raw) };
    }
}
unsafe extern "C" fn stream_state_changed_trampoline(
    data: *mut c_void,
    old: c_int,
    state: c_int,
    error: *const c_char,
) {
    let ctx = unsafe { &mut *(data as *mut StreamCtx) };
    if let Some(cb) = ctx.callbacks.state_changed.as_mut() {
        let err = if error.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(error) }
                    .to_string_lossy()
                    .into_owned(),
            )
        };
        cb(old, state, err);
    }
}
unsafe extern "C" fn stream_param_changed_trampoline(
    data: *mut c_void,
    id: u32,
    _param: *const c_void,
) {
    let ctx = unsafe { &mut *(data as *mut StreamCtx) };
    if let Some(cb) = ctx.callbacks.param_changed.as_mut() {
        cb(id);
    }
}

// Stream Manager

pub struct PwStream {
    pw: PipeWire,
    ptr: *mut c_void,
    listeners: Vec<RegisteredListener>,
}
unsafe impl Send for PwStream {}

impl PwStream {
    pub fn new(pw: &PipeWire, core: &PwCore, name: &str, props: PwProperties) -> Result<Self> {
        let name_c = CString::new(name)?;
        let ptr = unsafe { (pw.api.pw_stream_new)(core.ptr, name_c.as_ptr(), props.ptr) };
        if ptr.is_null() {
            return Err(anyhow!("pw_stream_new failed"));
        }
        std::mem::forget(props);
        Ok(Self {
            pw: pw.clone(),
            ptr,
            listeners: Vec::new(),
        })
    }

    pub fn add_listener(&mut self, callbacks: StreamCallbacks) -> Result<()> {
        let mut ctx = Box::new(StreamCtx {
            pw: self.pw.clone(),
            stream_ptr: self.ptr,
            callbacks,
        });
        let ctx_ptr = ctx.as_mut() as *mut StreamCtx as *mut c_void;

        let mut events = Box::new(pw_stream_events {
            version: PW_VERSION_STREAM_EVENTS,
            destroy: None,
            state_changed: Some(stream_state_changed_trampoline),
            control_info: None,
            io_changed: None,
            param_changed: Some(stream_param_changed_trampoline),
            add_buffer: None,
            remove_buffer: None,
            process: Some(stream_process_trampoline),
            drained: None,
        });
        let events_ptr = events.as_mut() as *mut pw_stream_events;

        let mut hook = Box::new(spa_hook::new());
        unsafe {
            (self.pw.api.pw_stream_add_listener)(self.ptr, hook.as_mut(), events_ptr, ctx_ptr)
        };
        self.listeners.push(RegisteredListener {
            _hook: hook,
            _keepalive: Box::new((events, ctx)),
        });
        Ok(())
    }

    pub fn connect(
        &self,
        direction: c_int,
        target_id: u32,
        flags: c_int,
        params: &[&[u8]],
    ) -> Result<()> {
        let ptrs: Vec<*const c_void> = params.iter().map(|p| p.as_ptr() as *const c_void).collect();
        let res = unsafe {
            (self.pw.api.pw_stream_connect)(
                self.ptr,
                direction,
                target_id,
                flags,
                ptrs.as_ptr(),
                ptrs.len() as u32,
            )
        };
        if res < 0 {
            return Err(anyhow!("pw_stream_connect failed: {res}"));
        }
        Ok(())
    }

    pub fn node_id(&self) -> u32 {
        unsafe { (self.pw.api.pw_stream_get_node_id)(self.ptr) }
    }
}

impl Drop for PwStream {
    fn drop(&mut self) {
        unsafe { (self.pw.api.pw_stream_destroy)(self.ptr) };
    }
}
