use std::mem::MaybeUninit;
use std::rc::Rc;

use x11::xrender::{XGlyphInfo, XRenderColor};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ColormapAlloc, ColormapWrapper, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask,
    WindowClass,
};
use x11rb::protocol::Event;

use x11rb::protocol::randr::{get_monitors, get_output_info, get_provider_info, get_providers};

use x11rb::xcb_ffi::XCBConnection as RawConnection;
use x11rb::{COPY_FROM_PARENT, CURRENT_TIME};

use x11::xft::*;
use x11::xlib::{XDefaultVisual, XFlush, XFree, XOpenDisplay, _XDisplay};

use cairo::{XCBConnection, XCBDrawable, XCBSurface, XCBVisualType, Context};

mod utils;
use utils::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _dpy = GenericFreeWrapper::new(
        unsafe { XOpenDisplay(std::ptr::null()) },
        xclosedisplay as fn(*mut _XDisplay) -> i32,
    );
    // let cvoid = unsafe { XGetXCBConnection(dpy) };

    let (conn, screen) = RawConnection::connect(None)?;

    monitors(&conn, screen)?;
    checking_colors(&conn, screen)?;
    fonts_using_cairo(&conn, screen)?;

    Ok(())
}

fn fonts_using_cairo(
    conn: &RawConnection,
    screen_num: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // create the window and other stuff
    let screen = &conn.setup().roots[screen_num];
    let (mut width, mut height) = (800, 600);
    let window = conn.generate_id()?;
    conn.create_window(
        screen.root_depth,
        window,
        screen.root,
        0,
        0,
        width,
        height,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &CreateWindowAux::new()
            .background_pixel(screen.white_pixel)
            .border_pixel(screen.black_pixel)
            .event_mask(EventMask::STRUCTURE_NOTIFY | EventMask::EXPOSURE | EventMask::KEY_PRESS),
    )?;
    conn.map_window(window)?;
    // connect to cairo
    let mut visual_ffi = find_xcb_visualtype(conn, screen.root_visual).unwrap();
    let cconn = unsafe { XCBConnection::from_raw_none(conn.get_raw_xcb_connection() as _) };
    let visual = unsafe { XCBVisualType::from_raw_none(&mut visual_ffi as *mut _ as _) };
    let surface = XCBSurface::create(
        &cconn,
        &XCBDrawable(window),
        &visual,
        width.into(),
        height.into(),
    )?;

    let fonts = vec!["Ubuntu Sans", "JetBrainsMono", "monospace", "Arial"];
    let mut index = 0;

    // cairo draw function, redraw on every expose event
    let mut draw = |cr: &cairo::Context, w: f32, h: f32| -> Result<(), Box<dyn std::error::Error>> {
        // cr.set_operator(cairo::Operator::Source);
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.rectangle(0.0, 0.0, w.into(), h.into());
        cr.fill()?;
        // cr.set_operator(cairo::Operator::Source);
        let text = "hello world";
        cr.set_source_rgb(0.8, 0.3, 0.7);
        cr.move_to(0 as f64, (h / 2.0) as f64);

        for char in text.chars() {
            let mut mod_char = char.to_string();
            if index + 1 == fonts.len() {
                index = 0
            } else {
                index += 1
            }

            if index % 2 == 0 {
                mod_char = char.to_uppercase().to_string();
            }
            cr.select_font_face(fonts[index], cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            cr.set_font_size(20 as _);
            cr.show_text(&format!("{mod_char} "))?;
            cr.rel_move_to(5.0, 0.into())
        }

        cr.move_to(0.0 as _, 20 as _);
        cr.set_font_size(16 as _);
        cr.show_text("hello ")?;
        cr.show_text("world ")?;
        cr.show_text("this ")?;
        cr.show_text("is ")?;
        cr.show_text("multiple ")?;
        cr.show_text("calls")?;

        Ok(())
    };

    loop {
        conn.flush()?;
        let mut redraw = false;
        let event = conn.wait_for_event()?;
        let mut event_option = Some(event);
        while let Some(event) = event_option {
            match event {
                Event::Expose(_) => {
                    redraw = true
                },
                Event::ConfigureNotify(e) => {
                    width = e.width as _;
                    height = e.height as _;
                    redraw = true;
                    surface.set_size(width as _, height as _)?;
                }
                Event::DestroyNotify(_) => {
                    return Ok(())
                }
                Event::KeyPress(e) => {
                    // println!("{e:#?}");
                    if e.detail == 9 {
                        return Ok(())
                    } else if e.detail == 57 {
                        redraw = true
                    }
                }
                _ => ()
            }
            event_option = conn.poll_for_event()?;
        }
        if redraw {
            let cr = Context::new(&surface)?;
            draw(&cr, width as _, height as _)?;
            surface.flush();
        }
    }
}

#[allow(unused)]
fn fonts(
    conn: &impl Connection,
    dpy: GenericFreeWrapper<_XDisplay, fn(*mut _XDisplay) -> i32>,
    screen: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let window_id = conn.generate_id()?;
    let gc = conn.generate_id()?;
    let font_name = "*".as_bytes();
    let fonts = conn.list_fonts(font_name.len() as _, font_name)?.reply()?;
    let _font_name = String::from_utf8(fonts.names[0].name.as_slice().to_vec())?;
    let dpy = Rc::new(dpy);

    let scr = &conn.setup().roots[screen];
    let visual = conn.setup().roots[screen].root_visual;
    let root = conn.setup().roots[screen].root;
    let colormap = ColormapWrapper::create_colormap(conn, ColormapAlloc::NONE, root, visual)?;

    let mut width = 800;
    let mut height = 600;
    let create_window = CreateWindowAux::new()
        .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY)
        .background_pixel(scr.white_pixel);

    conn.create_window(
        COPY_FROM_PARENT.try_into()?,
        window_id,
        conn.setup().roots[screen].root,
        0,
        0,
        width,
        height,
        0,
        WindowClass::INPUT_OUTPUT,
        visual,
        &create_window,
    )?;

    conn.create_gc(
        gc,
        window_id,
        &CreateGCAux::new()
            .foreground(scr.black_pixel)
            .background(scr.black_pixel),
    )?;

    let font = GenericXftWrapper::new(
        unsafe {
            XftFontOpenName(
                dpy.ptr(),
                screen as _,
                b"sans:bold:pixelsize=18\0".as_ptr() as _,
            )
        },
        dpy.clone(),
        xftfontclose,
    );
    if !font.ptr().is_null() {
        unsafe {
            println!("{:#?}", *font.ptr());
        };
    }
    let wm_delete_window = conn.intern_atom(false, b"WM_DELETE_WINDOW")?.reply()?.atom;

    let text = b"hello world\0";
    let text_len = text.len() as i32;
    let mut extents: MaybeUninit<XGlyphInfo> = MaybeUninit::uninit();

    unsafe {
        XftTextExtentsUtf8(
            dpy.ptr(),
            font.ptr(),
            text.as_ptr() as _,
            text_len,
            extents.as_mut_ptr(),
        )
    };
    if !extents.as_ptr().is_null() {
        println!(
            "Iosevka:size=12 text: '{}' extents: {:#?}\n",
            String::from_utf8(text.to_vec())?,
            unsafe { extents.assume_init() },
        );
    }

    let visual_xlib = unsafe { XDefaultVisual(dpy.ptr(), screen as _) };
    println!("visual {:#?}", unsafe { *visual_xlib });
    let render_color = &mut XRenderColor {
        alpha: 0xffff,
        red: 0xffff,
        green: 0x00ff,
        blue: 0x00ff,
    };
    let white_color = &mut XRenderColor {
        alpha: 0xffff,
        red: 0xffff,
        green: 0xffff,
        blue: 0x11ff,
    };
    let mut color: MaybeUninit<XftColor> = MaybeUninit::uninit();
    let mut c2: MaybeUninit<XftColor> = MaybeUninit::uninit();
    unsafe {
        XftColorAllocValue(
            dpy.ptr(),
            visual_xlib,
            colormap.colormap() as _,
            render_color,
            color.as_mut_ptr(),
        );
        XftColorAllocValue(
            dpy.ptr(),
            visual_xlib,
            colormap.colormap() as _,
            white_color,
            c2.as_mut_ptr(),
        );
        color.assume_init();
        c2.assume_init();
    };

    conn.map_window(window_id)?;
    conn.flush()?;
    let xft_draw = unsafe {
        XftDrawCreate(
            dpy.ptr(),
            window_id as _,
            visual as _,
            colormap.colormap().into(),
        )
    };

    loop {
        while let Some(event) = conn.poll_for_event()? {
            match event {
                Event::Expose(_) => {
                    unsafe {
                        XftDrawRect(
                            xft_draw,
                            color.as_ptr(),
                            (width / 2) as _,
                            (height / 2) as _,
                            extents.assume_init().width as _,
                            extents.assume_init().height as _,
                        );
                        XftDrawStringUtf8(
                            xft_draw,
                            color.as_ptr(),
                            font.ptr(),
                            (width / 2) as _,
                            (height / 2) as _,
                            text.as_ptr(),
                            text_len as _,
                        );
                        XFlush(dpy.ptr());
                    };
                    println!("exposed");
                }
                Event::ConfigureNotify(event) => {
                    width = event.width;
                    height = event.height;
                }
                Event::ClientMessage(event) => {
                    let data = event.data.as_data32();
                    if event.format == 32
                        && event.window == window_id
                        && data[0] == wm_delete_window
                    {
                        println!("Window was asked to close");
                        return Ok(());
                    }
                }
                Event::DestroyNotify(ev) => {
                    println!("Closing");
                    if ev.event == window_id {
                        unsafe {
                            XftColorFree(
                                dpy.ptr(),
                                visual_xlib,
                                colormap.colormap() as _,
                                color.as_mut_ptr(),
                            );
                            println!("Closed color 1");
                            XftColorFree(
                                dpy.ptr(),
                                visual_xlib,
                                colormap.colormap() as _,
                                c2.as_mut_ptr(),
                            );
                            println!("Closed color 2");
                            XftDrawDestroy(xft_draw as _);
                            println!("Closed draw");
                            XFree(visual_xlib as _);
                            println!("Closed visual");
                        };
                        break;
                    }
                }
                Event::Error(e) => println!("Got an unexpected error: {e:#?}"),
                _ => println!("Got an unknown event"),
            }
        }
    }
}

#[allow(unused)]
fn checking_colors(
    conn: &impl Connection,
    screen: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let screen = &conn.setup().roots[screen];
    // root window
    let root = screen.root;
    // root window visual
    let root_visual = screen.root_visual;
    // root depth
    let root_depth = screen.root_depth;
    // default_colormap
    let default_colormap = screen.default_colormap;
    // visual?
    let visual = &screen.allowed_depths[0];

    println!("root window: {root}\nroot visual: {root_visual}\nroot_depth: {root_depth}\n");
    // println!("allowed visuals: {:#?}", screen.allowed_depths);

    // query all colors
    let pixels: Vec<u32> = (0..1024).into_iter().collect();
    let colors = conn
        .query_colors(default_colormap, &pixels)?
        .reply()?
        .colors;

    // create a custom coloramp
    let colormap = ColormapWrapper::create_colormap(conn, ColormapAlloc::NONE, root, root_visual)?;

    // allocate a new color on the colormap created above
    let red = 0xcccc;
    let green = 0xbebe;
    let blue = 0x8181;
    let aloc_color_reply = conn
        .alloc_color(colormap.colormap(), red, green, blue)?
        .reply()?;

    let pixel = aloc_color_reply.pixel;

    println!(
        "color: {}\nr: {:#x}, g: {:#x}, b: {:#x}",
        pixel, aloc_color_reply.red, aloc_color_reply.green, aloc_color_reply.blue
    );

    assert_eq!(red, aloc_color_reply.red);
    assert_eq!(green, aloc_color_reply.green);
    assert_eq!(blue, aloc_color_reply.blue);

    // check for the color
    let query_colors_reply = conn.query_colors(colormap.colormap(), &[pixel])?.reply()?;
    assert!(!query_colors_reply.colors.is_empty());

    // free the color
    conn.free_colors(colormap.colormap(), pixel, &[pixel])?;

    // check that the color is freed
    let query_colors_reply2 = conn.query_colors(colormap.colormap(), &[pixel])?.reply()?;
    println!("{:#x?}", query_colors_reply2.colors);
    // assert!(query_colors_reply.colors.is_empty());

    Ok(())
}

#[allow(unused)]
fn monitors(conn: &impl Connection, screen: usize) -> Result<(), Box<dyn std::error::Error>> {
    let root = conn.setup().roots[screen].root;
    let monitors = get_monitors(conn, root, false)?.reply()?;
    let providers = get_providers(conn, root)?.reply()?;

    println!("Num monitors {:#?}", monitors.n_monitors());
    println!("first one = {:#?}", monitors.monitors[0]);
    println!("second one = {:#?}", monitors.monitors[1]);

    println!("listing providers:");
    for provider in providers.providers {
        let prov = get_provider_info(conn, provider, CURRENT_TIME)?.reply()?;

        let name = String::from_utf8(prov.name)?;
        println!("provider {} has name: {}", provider, name);

        for output in prov.outputs {
            let out = get_output_info(conn, output, CURRENT_TIME)?.reply()?;

            let name = String::from_utf8(out.name)?;
            println!("output {output} name {name}");
        }
    }

    println!("Listing monitors and output names");
    for monitor in monitors.monitors {
        for output in monitor.outputs {
            let out = get_output_info(conn, output, CURRENT_TIME)?.reply()?;

            let name = String::from_utf8(out.name)?;
            println!("output {output} name {name}");
        }
    }
    Ok(())
}
