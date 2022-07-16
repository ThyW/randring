#![allow(unused)]
#![allow(clippy::type_complexity)]
use std::ffi::c_void;
use std::rc::Rc;

use x11::xft::{XftFont, XftFontClose};
use x11::xlib::{XCloseDisplay, XFree, _XDisplay};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Visualid, Visualtype};
use x11rb::protocol::render::{ConnectionExt as _, PictType};

pub fn xfree(obj: *mut c_void) -> i32 {
    unsafe { XFree(obj) }
}

pub fn xclosedisplay(dpy: *mut _XDisplay) -> i32 {
    unsafe { XCloseDisplay(dpy) }
}

pub fn xftfontclose(dpy: *mut _XDisplay, font: *mut XftFont) {
    unsafe { XftFontClose(dpy, font) }
}

pub struct GenericFreeWrapper<T, F: Fn(*mut T) -> i32> {
    thing: *mut T,
    free: F,
}

impl<T, F: Fn(*mut T) -> i32> GenericFreeWrapper<T, F> {
    pub fn new(thing: *mut T, free: F) -> Self {
        Self { thing, free }
    }
    pub fn ptr(&self) -> *mut T {
        self.thing
    }
}

impl<T, F: Fn(*mut T) -> i32> Drop for GenericFreeWrapper<T, F> {
    fn drop(&mut self) {
        println!("dropping wrapper");
        (self.free)(self.thing);
    }
}

pub struct GenericXftWrapper<T, F: Fn(*mut _XDisplay, *mut T)> {
    thing: *mut T,
    dpy: Rc<GenericFreeWrapper<_XDisplay, fn(*mut _XDisplay) -> i32>>,
    free: F,
}

impl<T, F: Fn(*mut _XDisplay, *mut T)> GenericXftWrapper<T, F> {
    pub fn new(
        thing: *mut T,
        dpy: Rc<GenericFreeWrapper<_XDisplay, fn(*mut _XDisplay) -> i32>>,
        free: F,
    ) -> Self {
        Self { thing, dpy, free }
    }

    pub fn ptr(&self) -> *mut T {
        self.thing
    }
}

impl<T, F: Fn(*mut _XDisplay, *mut T)> Drop for GenericXftWrapper<T, F> {
    fn drop(&mut self) {
        println!("dropping xft");
        (self.free)(self.dpy.ptr(), self.thing)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct xcb_visualtype_t {
    pub visual_id: u32,
    pub class: u8,
    pub bits_per_rgb_value: u8,
    pub colormap_entries: u16,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub pad0: [u8; 4],
}

impl From<Visualtype> for xcb_visualtype_t {
    fn from(a: Visualtype) -> Self {
        Self {
            visual_id: a.visual_id,
            class: u8::from(a.class),
            bits_per_rgb_value: a.bits_per_rgb_value,
            colormap_entries: a.colormap_entries,
            red_mask: a.red_mask,
            green_mask: a.green_mask,
            blue_mask: a.blue_mask,
            pad0: [0; 4],
        }
    }
}

pub fn choose_visual(conn: &impl Connection, screen_num: usize) -> Result<(u8, Visualid), Box<dyn std::error::Error>> {
    let depth = 32;
    let screen = &conn.setup().roots[screen_num];

    // Try to use XRender to find a visual with alpha support
    let has_render = conn
        .extension_information(x11rb::protocol::render::X11_EXTENSION_NAME)?
        .is_some();
    if has_render {
        let formats = conn.render_query_pict_formats()?.reply()?;
        // Find the ARGB32 format that must be supported.
        let format = formats
            .formats
            .iter()
            .filter(|info| (info.type_, info.depth) == (PictType::DIRECT, depth))
            .filter(|info| {
                let d = info.direct;
                (d.red_mask, d.green_mask, d.blue_mask, d.alpha_mask) == (0xff, 0xff, 0xff, 0xff)
            })
            .find(|info| {
                let d = info.direct;
                (d.red_shift, d.green_shift, d.blue_shift, d.alpha_shift) == (16, 8, 0, 24)
            });
        if let Some(format) = format {
            // Now we need to find the visual that corresponds to this format
            if let Some(visual) = formats.screens[screen_num]
                .depths
                .iter()
                .flat_map(|d| &d.visuals)
                .find(|v| v.format == format.id)
            {
                return Ok((format.depth, visual.visual));
            }
        }
    }
    Ok((screen.root_depth, screen.root_visual))
}

pub fn find_xcb_visualtype(conn: &impl Connection, visual_id: u32) -> Option<xcb_visualtype_t> {
    for root in &conn.setup().roots {
        for depth in &root.allowed_depths {
            for visual in &depth.visuals {
                if visual.visual_id == visual_id {
                    return Some((*visual).into());
                }
            }
        }
    }
    None
}
