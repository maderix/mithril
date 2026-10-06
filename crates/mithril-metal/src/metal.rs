//! Metal over the Objective-C runtime: `objc_msgSend` cast to each
//! method's signature. Objects this module creates with `new...` are owned
//! (released on drop); autoreleased ones live inside one pool per run.

use std::ffi::{c_char, c_void, CStr, CString};

type Id = *mut c_void;
type Sel = *mut c_void;

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

#[link(name = "Metal", kind = "framework")]
extern "C" {
    fn MTLCreateSystemDefaultDevice() -> Id;
}

#[link(name = "Foundation", kind = "framework")]
extern "C" {}

fn sel(s: &str) -> Sel {
    let c = CString::new(s).unwrap();
    unsafe { sel_registerName(c.as_ptr()) }
}

fn class(s: &str) -> Id {
    let c = CString::new(s).unwrap();
    unsafe { objc_getClass(c.as_ptr()) }
}

/// `[obj sel: a ...]` with the argument and result types spelled out.
macro_rules! msg {
    ($ret:ty; $obj:expr, $sel:expr $(, $a:expr => $t:ty)*) => {{
        let f: unsafe extern "C" fn(Id, Sel $(, $t)*) -> $ret =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f($obj, sel($sel) $(, $a)*) }
    }};
}

fn nsstring(s: &str) -> Id {
    let c = CString::new(s).unwrap();
    msg!(Id; class("NSString"), "stringWithUTF8String:", c.as_ptr() => *const c_char)
}

fn utf8(s: Id) -> String {
    if s.is_null() {
        return String::new();
    }
    let p: *const c_char = msg!(*const c_char; s, "UTF8String");
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn error_text(err: Id) -> String {
    utf8(msg!(Id; err, "localizedDescription"))
}

fn release(obj: Id) {
    if !obj.is_null() {
        msg!((); obj, "release");
    }
}

/// Runs `f` inside an autorelease pool.
fn pooled<T>(f: impl FnOnce() -> T) -> T {
    let pool = unsafe { objc_autoreleasePoolPush() };
    let r = f();
    unsafe { objc_autoreleasePoolPop(pool) };
    r
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    w: usize,
    h: usize,
    d: usize,
}

/// The system GPU and one command queue.
pub struct Device {
    dev: Id,
    queue: Id,
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Device {
    pub fn new() -> Option<Device> {
        let dev = unsafe { MTLCreateSystemDefaultDevice() };
        if dev.is_null() {
            return None;
        }
        let queue = msg!(Id; dev, "newCommandQueue");
        Some(Device { dev, queue })
    }

    pub fn name(&self) -> String {
        pooled(|| utf8(msg!(Id; self.dev, "name")))
    }

    /// Compile MSL source: no fast math (IEEE semantics, NaN-aware
    /// comparisons); the source itself turns contraction off.
    pub fn compile(&self, src: &str) -> Result<Library, String> {
        pooled(|| {
            let opts = msg!(Id; class("MTLCompileOptions"), "new");
            // MTLMathModeSafe
            msg!((); opts, "setMathMode:", 0isize => isize);
            let mut err: Id = std::ptr::null_mut();
            let lib = msg!(Id; self.dev, "newLibraryWithSource:options:error:", nsstring(src) => Id, opts => Id, &mut err => *mut Id);
            release(opts);
            if lib.is_null() {
                return Err(error_text(err));
            }
            Ok(Library(lib))
        })
    }

    /// The pipeline for kernel `name`, with `depth` frames of call stack
    /// for functions that recurse.
    pub fn pipeline(&self, lib: &Library, name: &str, depth: usize) -> Result<Pipeline, String> {
        pooled(|| {
            let f = msg!(Id; lib.0, "newFunctionWithName:", nsstring(name) => Id);
            if f.is_null() {
                return Err(format!("no kernel named {name}"));
            }
            let desc = msg!(Id; class("MTLComputePipelineDescriptor"), "new");
            msg!((); desc, "setComputeFunction:", f => Id);
            msg!((); desc, "setMaxCallStackDepth:", depth => usize);
            let mut err: Id = std::ptr::null_mut();
            let pso = msg!(Id; self.dev, "newComputePipelineStateWithDescriptor:options:reflection:error:",
                desc => Id, 0usize => usize, std::ptr::null_mut::<Id>() => *mut Id, &mut err => *mut Id);
            release(desc);
            release(f);
            if pso.is_null() {
                return Err(error_text(err));
            }
            Ok(Pipeline(pso))
        })
    }

    /// A zeroed buffer of `bytes` in memory the CPU and the GPU share.
    pub fn buffer(&self, bytes: usize) -> Buffer {
        // MTLResourceStorageModeShared
        let id = msg!(Id; self.dev, "newBufferWithLength:options:", bytes.max(1) => usize, 0usize => usize);
        assert!(!id.is_null(), "Metal could not allocate {bytes} bytes");
        let ptr = msg!(*mut u8; id, "contents");
        unsafe { std::ptr::write_bytes(ptr, 0, bytes) };
        Buffer { id, ptr, len: bytes }
    }

    /// Run the dispatches in order, in one command buffer, and wait. A
    /// dispatch boundary is a full barrier for device memory.
    pub fn run(&self, dispatches: &[Dispatch]) -> Result<Timing, String> {
        self.submit(dispatches).wait()
    }

    /// Start the dispatches (one command buffer) without waiting.
    pub fn submit(&self, dispatches: &[Dispatch]) -> Submitted {
        pooled(|| {
            let t0 = std::time::Instant::now();
            let cb = msg!(Id; self.queue, "commandBuffer");
            // kept past this pool: released when the Submitted drops
            msg!(Id; cb, "retain");
            let enc = msg!(Id; cb, "computeCommandEncoder");
            for d in dispatches {
                msg!((); enc, "setComputePipelineState:", d.pipeline.0 => Id);
                for (i, b) in d.buffers.iter().enumerate() {
                    msg!((); enc, "setBuffer:offset:atIndex:", b.id => Id, 0usize => usize, i => usize);
                }
                // MTLResourceUsageRead | MTLResourceUsageWrite
                for b in &d.reached {
                    msg!((); enc, "useResource:usage:", b.id => Id, 3usize => usize);
                }
                let threads = Size { w: d.threads, h: 1, d: 1 };
                let group = Size { w: d.group, h: 1, d: 1 };
                msg!((); enc, "dispatchThreads:threadsPerThreadgroup:", threads => Size, group => Size);
            }
            msg!((); enc, "endEncoding");
            msg!((); cb, "commit");
            Submitted { cb, t0 }
        })
    }
}

/// What a command buffer cost: from submission to completion, and on the
/// GPU (nanoseconds).
#[derive(Clone, Copy, Debug, Default)]
pub struct Timing {
    pub wall_ns: f64,
    pub gpu_ns: f64,
}

/// A command buffer on its way through the GPU.
pub struct Submitted {
    cb: Id,
    t0: std::time::Instant,
}

unsafe impl Send for Submitted {}

impl Submitted {
    /// The GPU has finished it (completed or failed).
    pub fn done(&self) -> bool {
        // MTLCommandBufferStatusCompleted = 4, MTLCommandBufferStatusError = 5
        msg!(usize; self.cb, "status") >= 4
    }

    pub fn wait(self) -> Result<Timing, String> {
        pooled(|| {
            msg!((); self.cb, "waitUntilCompleted");
            let err = msg!(Id; self.cb, "error");
            if !err.is_null() {
                return Err(error_text(err));
            }
            let gpu = msg!(f64; self.cb, "GPUEndTime") - msg!(f64; self.cb, "GPUStartTime");
            Ok(Timing { wall_ns: self.t0.elapsed().as_nanos() as f64, gpu_ns: gpu * 1e9 })
        })
    }
}

impl Drop for Submitted {
    fn drop(&mut self) {
        release(self.cb);
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        release(self.queue);
        release(self.dev);
    }
}

pub struct Library(Id);

// Libraries and pipeline states are immutable once made, and Metal allows
// them on any thread.
unsafe impl Send for Library {}
unsafe impl Sync for Library {}

impl Drop for Library {
    fn drop(&mut self) {
        release(self.0);
    }
}

pub struct Pipeline(Id);

unsafe impl Send for Pipeline {}
unsafe impl Sync for Pipeline {}

impl Pipeline {
    /// The most threads one threadgroup of this pipeline may have.
    pub fn max_group(&self) -> usize {
        msg!(usize; self.0, "maxTotalThreadsPerThreadgroup")
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        release(self.0);
    }
}

pub struct Buffer {
    id: Id,
    ptr: *mut u8,
    len: usize,
}

unsafe impl Send for Buffer {}

impl Buffer {
    pub fn bytes(&self) -> usize {
        self.len
    }

    /// The buffer as words. Valid between runs (no dispatch is writing).
    pub fn words(&self) -> &[u64] {
        unsafe { std::slice::from_raw_parts(self.ptr as *const u64, self.len / 8) }
    }

    /// The buffer's address in the GPU's address space (a kernel may turn
    /// it into a pointer; the buffer must be bound to the dispatch).
    pub fn gpu_address(&self) -> u64 {
        msg!(u64; self.id, "gpuAddress")
    }

    pub fn words_mut(&mut self) -> &mut [u64] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr as *mut u64, self.len / 8) }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        release(self.id);
    }
}

/// One kernel launch: `threads` threads in groups of `group`, with the
/// buffers bound at indices 0, 1, ...; `reached` are buffers the kernel
/// reads and writes through device addresses only (made resident).
pub struct Dispatch<'a> {
    pub pipeline: &'a Pipeline,
    pub buffers: Vec<&'a Buffer>,
    pub reached: Vec<&'a Buffer>,
    pub threads: usize,
    pub group: usize,
}
