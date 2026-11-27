#![expect(
    unsafe_code,
    reason = "ScreenCaptureKit frame buffers and lifetime cross the native C ABI"
)]

use anyhow::{Result, bail};
use ironrdp_server::{
    BitmapUpdate, DesktopSize, DisplayUpdate, PixelFormat, RdpServerDisplayUpdates, ServerError,
    ServerErrorExt, ServerResult,
};
use std::{
    ffi::c_void,
    num::{NonZeroU16, NonZeroUsize},
    ptr::NonNull,
};
use tokio::sync::watch;

type Frames = watch::Sender<Option<BitmapUpdate>>;

unsafe extern "C" {
    fn anima_rdp_permissions() -> u8;
    fn anima_rdp_request_permissions() -> u8;
    fn anima_rdp_main_display() -> u32;
    fn anima_rdp_display_width(display: u32) -> u32;
    fn anima_rdp_display_height(display: u32) -> u32;
    fn anima_rdp_capture(
        receive: extern "C" fn(*mut c_void, *const u8, usize, usize, usize),
        release: extern "C" fn(*mut c_void),
        context: *mut c_void,
        display: u32,
        width: u16,
        height: u16,
    ) -> *mut c_void;
    fn anima_rdp_stop_capture(handle: *mut c_void);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permissions {
    pub screen: bool,
    pub input: bool,
}

fn flags(value: u8) -> Permissions {
    Permissions {
        screen: value & 1 != 0,
        input: value & 2 != 0,
    }
}

#[must_use]
pub fn permissions() -> Permissions {
    // The bridge only checks authorization flags for this process.
    flags(unsafe { anima_rdp_permissions() })
}

#[must_use]
pub fn request_permissions() -> Permissions {
    // The caller presents these system prompts from the foreground app.
    flags(unsafe { anima_rdp_request_permissions() })
}

#[derive(Clone, Copy)]
pub struct Desktop {
    pub id: u32,
    pub size: DesktopSize,
}

impl Desktop {
    pub fn main() -> Result<Self> {
        if permissions()
            != (Permissions {
                screen: true,
                input: true,
            })
        {
            bail!("Screen Recording and input permissions are required");
        }
        // These Core Graphics calls only read the current display.
        let (id, width, height) = unsafe {
            let id = anima_rdp_main_display();
            (
                id,
                anima_rdp_display_width(id),
                anima_rdp_display_height(id),
            )
        };
        if width == 0 || height == 0 {
            bail!("no main display");
        }
        let width = u64::from(width);
        let height = u64::from(height);
        let bounds = if width * 720 > height * 1280 {
            (width.min(1280), height * width.min(1280) / width)
        } else {
            (width * height.min(720) / height, height.min(720))
        };
        let to_even = |pixels: u64| u16::try_from(pixels.max(4)).unwrap_or(4) & !1;
        Ok(Self {
            id,
            size: DesktopSize {
                width: to_even(bounds.0),
                height: to_even(bounds.1),
            },
        })
    }
}

pub struct Capture {
    handle: NonNull<c_void>,
    frames: watch::Receiver<Option<BitmapUpdate>>,
}

// The handle has unique ownership. ScreenCaptureKit callbacks use the bridge's
// serial queue, and dropping the handle removes and drains that output.
unsafe impl Send for Capture {}

impl Capture {
    pub fn start(desktop: Desktop) -> Result<Self> {
        let (send, frames) = watch::channel(None);
        let context = Box::into_raw(Box::new(send)).cast();
        // The bridge owns the context even when capture fails.
        let handle = unsafe {
            anima_rdp_capture(
                receive,
                release,
                context,
                desktop.id,
                desktop.size.width,
                desktop.size.height,
            )
        };
        let Some(handle) = NonNull::new(handle) else {
            bail!("screen capture did not start");
        };
        Ok(Self { handle, frames })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // Calls are serialized by the native stream; stopping drains its callback queue.
        unsafe { anima_rdp_stop_capture(self.handle.as_ptr()) };
    }
}

#[async_trait::async_trait]
impl RdpServerDisplayUpdates for Capture {
    async fn next_update(&mut self) -> ServerResult<Option<DisplayUpdate>> {
        self.frames
            .changed()
            .await
            .map_err(|_| ServerError::channel("screen capture ended"))?;
        Ok(self
            .frames
            .borrow_and_update()
            .clone()
            .map(DisplayUpdate::Bitmap))
    }
}

extern "C" fn receive(
    context: *mut c_void,
    pixels: *const u8,
    width: usize,
    height: usize,
    stride: usize,
) {
    if !(1..=1280).contains(&width)
        || !(1..=720).contains(&height)
        || stride < width * 4
        || stride > 8192
    {
        return;
    }
    let Some(width) = u16::try_from(width).ok().and_then(NonZeroU16::new) else {
        return;
    };
    let Some(height) = u16::try_from(height).ok().and_then(NonZeroU16::new) else {
        return;
    };
    let Some(stride) = NonZeroUsize::new(stride) else {
        return;
    };
    let count = usize::from(height.get()) * stride.get();
    // ScreenCaptureKit keeps the locked buffer until this callback returns.
    let bytes = unsafe { std::slice::from_raw_parts(pixels, count) }.to_vec();
    // The callback queue cannot run after the bridge releases this context.
    let send = unsafe { &*context.cast::<Frames>() };
    send.send_replace(Some(BitmapUpdate {
        x: 0,
        y: 0,
        width,
        height,
        format: PixelFormat::BgrA32,
        data: bytes.into(),
        stride,
    }));
}

extern "C" fn release(context: *mut c_void) {
    // The native capture object calls this exactly once, in dealloc.
    drop(unsafe { Box::from_raw(context.cast::<Frames>()) });
}
