//!
//! Egl [`NativeDisplay`](::backend::egl::native::NativeDisplay) and
//! [`NativeSurface`](::backend::egl::native::NativeSurface) support for
//! [`GbmDevice`](GbmDevice) and [`GbmSurface`](GbmSurface).
//!

use crate::backend::drm::{Device, RawDevice};
use crate::backend::egl::error::Result as EglResult;
use crate::backend::egl::ffi;
use crate::backend::egl::native::{Backend, NativeDisplay, NativeSurface};
use crate::backend::graphics::SwapBuffersError;

use super::error::{Error, Result};
use super::{GbmDevice, GbmSurface};

use drm::control::{crtc, Device as ControlDevice};
use gbm::AsRaw;
use std::marker::PhantomData;
use std::ptr;

/// Egl Gbm backend type
///
/// See [`Backend`](::backend::egl::native::Backend).
pub struct Gbm<D: RawDevice + 'static> {
    _userdata: PhantomData<D>,
}

impl<D: RawDevice + 'static> Backend for Gbm<D> {
    type Surface = GbmSurface<D>;

    unsafe fn get_display<F>(
        display: ffi::NativeDisplayType,
        has_dp_extension: F,
        log: ::slog::Logger,
    ) -> ffi::egl::types::EGLDisplay
    where
        F: Fn(&str) -> bool,
    {
        if has_dp_extension("EGL_KHR_platform_gbm") && ffi::egl::GetPlatformDisplay::is_loaded() {
            trace!(log, "EGL Display Initialization via EGL_KHR_platform_gbm");
            ffi::egl::GetPlatformDisplay(ffi::egl::PLATFORM_GBM_KHR, display as *mut _, ptr::null())
        } else if has_dp_extension("EGL_MESA_platform_gbm") && ffi::egl::GetPlatformDisplayEXT::is_loaded() {
            trace!(log, "EGL Display Initialization via EGL_MESA_platform_gbm");
            ffi::egl::GetPlatformDisplayEXT(ffi::egl::PLATFORM_GBM_MESA, display as *mut _, ptr::null())
        } else if has_dp_extension("EGL_MESA_platform_gbm") && ffi::egl::GetPlatformDisplay::is_loaded() {
            trace!(log, "EGL Display Initialization via EGL_MESA_platform_gbm");
            ffi::egl::GetPlatformDisplay(ffi::egl::PLATFORM_GBM_MESA, display as *mut _, ptr::null())
        } else {
            trace!(log, "Default EGL Display Initialization via GetDisplay");
            ffi::egl::GetDisplay(display as *mut _)
        }
    }
}

unsafe impl<D: RawDevice + ControlDevice + 'static> NativeDisplay<Gbm<D>> for GbmDevice<D> {
    type Arguments = crtc::Handle;
    type Error = Error;

    fn is_backend(&self) -> bool {
        true
    }

    fn ptr(&self) -> EglResult<ffi::NativeDisplayType> {
        Ok(self.dev.borrow().as_raw() as *const _)
    }

    fn create_surface(&mut self, crtc: crtc::Handle) -> Result<GbmSurface<D>> {
        Device::create_surface(self, crtc)
    }
}

unsafe impl<D: RawDevice + 'static> NativeSurface for GbmSurface<D> {
    fn ptr(&self) -> ffi::NativeWindowType {
        self.0.surface.borrow().as_raw() as *const _
    }

    fn needs_recreation(&self) -> bool {
        self.needs_recreation()
    }

    fn recreate(&self) -> bool {
        if let Err(err) = GbmSurface::recreate(self) {
            error!(self.0.logger, "Failure recreating internal resources: {:?}", err);
            false
        } else {
            true
        }
    }

    fn swap_buffers(&self) -> ::std::result::Result<(), SwapBuffersError> {
        // this is safe since `eglSwapBuffers` will have been called exactly once
        // if this is used by our egl module, which is why this trait is unsafe.
        unsafe { self.page_flip() }
    }
}
