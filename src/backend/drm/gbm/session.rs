//!
//! Support to register a [`GbmDevice`](GbmDevice)
//! to an open [`Session`](::backend::session::Session).
//!

use drm::control::{crtc, Device as ControlDevice, ResourceInfo};
use gbm::BufferObject;
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::rc::{Rc, Weak};

use super::{GbmDevice, GbmSurfaceInternal};
use backend::drm::{RawDevice, RawSurface};
use backend::session::{AsSessionObserver, SessionObserver};

/// [`SessionObserver`](SessionObserver)
/// linked to the [`GbmDevice`](GbmDevice) it was
/// created from.
pub struct GbmDeviceObserver<
    S: SessionObserver + 'static,
    D: RawDevice + ControlDevice + AsSessionObserver<S> + 'static,
> {
    observer: S,
    backends: Weak<RefCell<HashMap<crtc::Handle, Weak<GbmSurfaceInternal<D>>>>>,
    logger: ::slog::Logger,
}

impl<S: SessionObserver + 'static, D: RawDevice + ControlDevice + AsSessionObserver<S> + 'static>
    AsSessionObserver<GbmDeviceObserver<S, D>> for GbmDevice<D>
{
    fn observer(&mut self) -> GbmDeviceObserver<S, D> {
        GbmDeviceObserver {
            observer: (**self.dev.borrow_mut()).observer(),
            backends: Rc::downgrade(&self.backends),
            logger: self.logger.clone(),
        }
    }
}

impl<S: SessionObserver + 'static, D: RawDevice + ControlDevice + AsSessionObserver<S> + 'static>
    SessionObserver for GbmDeviceObserver<S, D>
{
    fn pause(&mut self, devnum: Option<(u32, u32)>) {
        self.observer.pause(devnum);
    }

    fn activate(&mut self, devnum: Option<(u32, u32, Option<RawFd>)>) {
        self.observer.activate(devnum);
        let mut crtcs = Vec::new();
        if let Some(backends) = self.backends.upgrade() {
            for (crtc, backend) in backends.borrow().iter() {
                if let Some(backend) = backend.upgrade() {
                    // restart rendering loop, if it was previously running
                    if let Some(Err(err)) = backend
                        .current_frame_buffer
                        .get()
                        .map(|fb| backend.crtc.page_flip(fb.handle()))
                    {
                        warn!(self.logger, "Failed to restart rendering loop. Error: {}", err);
                    }
                    // reset cursor
                    {
                        let &(ref cursor, ref hotspot): &(BufferObject<()>, (u32, u32)) =
                            unsafe { &*backend.cursor.as_ptr() };
                        if crtc::set_cursor2(
                            &*backend.dev.borrow(),
                            *crtc,
                            cursor,
                            ((*hotspot).0 as i32, (*hotspot).1 as i32),
                        )
                        .is_err()
                        {
                            if let Err(err) = crtc::set_cursor(&*backend.dev.borrow(), *crtc, cursor) {
                                error!(self.logger, "Failed to reset cursor. Error: {}", err);
                            }
                        }
                    }
                } else {
                    crtcs.push(*crtc);
                }
            }
            for crtc in crtcs {
                backends.borrow_mut().remove(&crtc);
            }
        }
    }
}
