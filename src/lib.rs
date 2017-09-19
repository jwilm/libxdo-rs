//! Rust API for a few libxdo methods
extern crate libxdo_sys as ffi;
extern crate libc;
extern crate x11;
#[macro_use]
extern crate foreign_types;

use std::ffi::{CStr, CString};
use std::ptr;
use std::time::Duration;

use foreign_types::ForeignTypeRef;
use libc::c_int;
use libc::useconds_t;
use x11::xlib::XFree;

const XDO_SUCCESS: c_int = 0;
const XDO_ERROR: c_int = 1;

pub struct CharcodeMapList {
    ptr: *mut ffi::Struct_charcodemap,
    len: c_int,
}

impl Drop for CharcodeMapList {
    fn drop(&mut self) {
        if self.ptr != ptr::null_mut() {
            unsafe {
                ::libc::free(self.ptr as *mut _);
            }
        }
    }
}

/// Handle for the `xdo` API
foreign_type! {
    type CType = ffi::xdo_t;
    fn drop = ffi::xdo_free;

    /// Wraps an instance of the `xdo` library
    pub struct Xdo;

    /// Borrowed version of `Xdo`
    pub struct XdoRef;
}

#[derive(Debug)]
pub enum Error {
    /// Some xdo call failed
    Failed(&'static str),

    /// Got a non-UTF-8 value
    Utf8(::std::str::Utf8Error),

    /// Passed a string parameter with a Null byte
    NullByteInString(::std::ffi::NulError),
}

pub type Result<T> = ::std::result::Result<T, Error>;

pub struct Window<'a> {
    id: x11::xlib::Window,
    xdo: &'a XdoRef,
}

use std::fmt;
impl<'a> fmt::Debug for Window<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Window")
            .field("id", &self.id)
            .finish()
    }
}

impl ::std::error::Error for Error {
    fn cause(&self) -> Option<&::std::error::Error> {
        match *self {
            Error::Utf8(ref err) => Some(err),
            Error::NullByteInString(ref err) => Some(err),
            _ => None,
        }
    }
    fn description(&self) -> &str {
        match *self {
            Error::Failed(ref _s) => "libxdo call returned error",
            Error::Utf8(ref err) => err.description(),
            Error::NullByteInString(_) => "string argument had a null byte",
        }
    }
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            Error::Failed(ref s) => write!(f, "libxdo::{} returned an error", s),
            Error::Utf8(ref err) => write!(f, "{}", err),
            Error::NullByteInString(ref err) => {
                write!(f, "A String argument containing a NULL byte was provided: {}", err)
            },
        }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(val: std::str::Utf8Error) -> Error {
        Error::Utf8(val)
    }
}

impl From<std::ffi::NulError> for Error {
    fn from(val: std::ffi::NulError) -> Error {
        Error::NullByteInString(val)
    }
}

fn ptr_or_error<T>(ptr: *mut T, method: &'static str) -> Result<*mut T> {
    if ptr.is_null() {
        Err(Error::Failed(method))
    } else {
        Ok(ptr)
    }
}

impl Xdo {
    pub fn new() -> Result<Xdo> {
        Ok(Xdo(ptr_or_error(unsafe { ffi::xdo_new(ptr::null()) }, "xdo_new")?))
    }
}

impl XdoRef {
    pub fn get_active_window(&self) -> Result<Window> {
        let mut ptr: x11::xlib::Window = 0;
        let res = unsafe {
            ffi::xdo_get_active_window(self.as_ptr(), &mut ptr)
        };

        match res {
            XDO_SUCCESS => Ok(Window { id: ptr, xdo: self }),
            XDO_ERROR => Err(Error::Failed("get_active_window")),
            _ => unreachable!()
        }
    }

    pub fn get_active_modifiers(&self) -> Result<CharcodeMapList> {
        let mut list = CharcodeMapList {
            ptr: ptr::null_mut(),
            len: 0,
        };

        let res = unsafe {
            ffi::xdo_get_active_modifiers(self.as_ptr(), &mut list.ptr, &mut list.len)
        };

        match res {
            XDO_SUCCESS => Ok(list),
            XDO_ERROR => Err(Error::Failed("get_active_modifiers")),
            _ => unreachable!()
        }
    }
}

impl<'a> Window<'a> {
    pub fn get_name(&self) -> Result<String> {
        let mut name: *mut libc::c_uchar = ptr::null_mut();
        let mut length: c_int = 0;
        let mut name_type: c_int = 0;

        let res = unsafe {
            ffi::xdo_get_window_name(
                self.xdo.as_ptr(),
                self.id,
                &mut name,
                &mut length,
                &mut name_type
            )
        };

        match res {
            XDO_SUCCESS => Ok({
                let rust_name = {
                    let cstr = unsafe { CStr::from_ptr(name as *const _) };
                    cstr.to_str()?.to_owned()
                };

                unsafe {
                    XFree(name as _);
                }

                rust_name
            }),
            XDO_ERROR => Err(Error::Failed("get_window_name")),
            _ => unreachable!()
        }
    }

    /// Send a keysequence to the specified window
    ///
    /// The delay is convereted to microseconds internally before forwarding to libxdo. If the delay
    /// in useconds exceeds useconds_t capacity, it will be truncated.
    pub fn send_keysequence(&self, sequence: &str, delay: Option<Duration>) -> Result<()> {
        let udelay: useconds_t = delay.map(|delay| {
            (delay.as_secs() as useconds_t * 1_000_000)
                + delay.subsec_nanos() as useconds_t / 1_000
        }).unwrap_or(0);

        let res = unsafe {
            let sequence = CString::new(sequence)?;
            ffi::xdo_send_keysequence_window(self.xdo.as_ptr(), self.id, sequence.as_ptr(), udelay)
        };

        match res {
            XDO_SUCCESS => Ok(()),
            XDO_ERROR => Err(Error::Failed("send_keysequence")),
            _ => unreachable!(),
        }
    }

    pub fn set_active_modifiers(&self, mods: &CharcodeMapList) -> Result<()> {
        let res = unsafe {
            ffi::xdo_set_active_modifiers(self.xdo.as_ptr(), self.id, mods.ptr, mods.len)
        };

        match res {
            XDO_SUCCESS => Ok(()),
            XDO_ERROR => Err(Error::Failed("set_active_modifiers")),
            _ => unreachable!()
        }
    }

    pub fn clear_active_modifiers(&self, mods: &CharcodeMapList) -> Result<()> {
        let res = unsafe {
            ffi::xdo_clear_active_modifiers(self.xdo.as_ptr(), self.id, mods.ptr, mods.len)
        };

        match res {
            XDO_SUCCESS => Ok(()),
            XDO_ERROR => Err(Error::Failed("clear_active_modifiers")),
            _ => unreachable!()
        }
    }
}


#[cfg(test)]
mod tests {
    use super::Xdo;

    #[test]
    fn get_window_name() {
        let xdo = Xdo::new().unwrap();
        let window = xdo.get_active_window().unwrap();
        let _name = window.get_name().unwrap();
    }

    #[test]
    fn send_keysequence() {
        let xdo = Xdo::new().unwrap();
        let window = xdo.get_active_window().unwrap();
        window.send_keysequence("Return", None).unwrap();
    }

    #[test]
    fn modifiers() {
        let xdo = Xdo::new().unwrap();
        let window = xdo.get_active_window().unwrap();
        let mods = xdo.get_active_modifiers().expect("get_active_modifiers");
        window.clear_active_modifiers(&mods).expect("clear_active_modifiers");
        window.set_active_modifiers(&mods).expect("set_active_modifiers");
    }
}
