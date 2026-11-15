//! Gives Chromium's X11 window the keyboard focus.
//!
//! The CEF runtime runs the browser in windowed mode: Chromium creates an
//! override-redirect X11 window, reparents it under the GTK window's host window, and
//! handles pointer and key input itself. X delivers key events to whichever window holds
//! the input focus, and a window manager never focuses an override-redirect child, so the
//! focus stays on the GTK toplevel (or on GTK's own 1×1 focus window) and every keystroke
//! is lost before it reaches the page: typing, IME composition and all shortcuts.
//!
//! `CefBrowserHost::SetFocus` (what `Webview::set_focus` calls) makes Chromium believe it
//! is focused — `document.hasFocus()` reports true — but it does not move the X focus, so
//! this finds this process's own window through `_NET_WM_PID`, walks down to Chromium's
//! window and focuses it.

use x11_dl::xlib::{Display, Window, Xlib};

type Pid = u64;

/// Focuses the largest Chromium browser window of this process.
///
/// Returns `false` when the process has no mapped window yet (the browser window appears
/// about a second after the GTK window), in which case nothing is changed.
pub fn focus_browser() -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("DISPLAY").is_none() {
        return false;
    }
    let Ok(xlib) = Xlib::open() else { return false };
    // SAFETY: every call passes the display opened here and window identifiers that came
    // back from the server; the display is closed before returning.
    unsafe {
        let display = (xlib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            return false;
        }
        let browser = our_windows(&xlib, display)
            .into_iter()
            .filter_map(|(toplevel, area)| browser_under(&xlib, display, toplevel).map(|b| (b, area)))
            .max_by_key(|(_, area)| *area)
            .map(|(browser, _)| browser);
        if let Some(browser) = browser {
            (xlib.XSetInputFocus)(display, browser, x11_dl::xlib::RevertToParent, x11_dl::xlib::CurrentTime);
            (xlib.XSync)(display, 0);
        }
        (xlib.XCloseDisplay)(display);
        browser.is_some()
    }
}

/// Every mapped top-level window of this process, with its pixel area.
unsafe fn our_windows(xlib: &Xlib, display: *mut Display) -> Vec<(Window, i64)> {
    unsafe {
        let pid = std::process::id() as Pid;
        let atom = (xlib.XInternAtom)(display, c"_NET_WM_PID".as_ptr(), 0);
        let root = (xlib.XDefaultRootWindow)(display);
        let (mut root_return, mut parent) = (0 as Window, 0 as Window);
        let (mut children, mut count) = (std::ptr::null_mut::<Window>(), 0u32);
        if (xlib.XQueryTree)(display, root, &mut root_return, &mut parent, &mut children, &mut count) == 0
            || children.is_null()
        {
            return Vec::new();
        }
        let kids = std::slice::from_raw_parts(children, count as usize);
        let mut found = Vec::new();
        for kid in kids {
            if window_pid(xlib, display, *kid, atom) == Some(pid)
                && let Some(area) = area(xlib, display, *kid)
            {
                found.push((*kid, area));
            }
        }
        (xlib.XFree)(children.cast());
        found
    }
}

/// `_NET_WM_PID` of `window`, if the window sets it.
unsafe fn window_pid(xlib: &Xlib, display: *mut Display, window: Window, atom: x11_dl::xlib::Atom) -> Option<Pid> {
    unsafe {
        if atom == 0 {
            return None;
        }
        let (mut actual_type, mut actual_format) = (0, 0);
        let (mut nitems, mut bytes_after) = (0u64, 0u64);
        let mut data: *mut u8 = std::ptr::null_mut();
        let status = (xlib.XGetWindowProperty)(
            display,
            window,
            atom,
            0,
            1,
            0,
            x11_dl::xlib::XA_CARDINAL,
            &mut actual_type,
            &mut actual_format,
            &mut nitems,
            &mut bytes_after,
            &mut data,
        );
        if status != 0 || data.is_null() {
            return None;
        }
        // A 32-bit CARDINAL comes back as one `long`.
        let value = (actual_format == 32 && nitems >= 1).then(|| *(data as *const Pid));
        (xlib.XFree)(data.cast());
        value
    }
}

/// Follows the largest mapped child down from `toplevel` to Chromium's browser window.
unsafe fn browser_under(xlib: &Xlib, display: *mut Display, toplevel: Window) -> Option<Window> {
    unsafe {
        let mut current = toplevel;
        let mut current_area = area(xlib, display, toplevel)?;
        for _ in 0..8 {
            let (mut root, mut parent) = (0 as Window, 0 as Window);
            let (mut children, mut count) = (std::ptr::null_mut::<Window>(), 0u32);
            if (xlib.XQueryTree)(display, current, &mut root, &mut parent, &mut children, &mut count) == 0
                || children.is_null()
                || count == 0
            {
                if !children.is_null() {
                    (xlib.XFree)(children.cast());
                }
                return (current != toplevel).then_some(current);
            }
            let kids = std::slice::from_raw_parts(children, count as usize);
            let mut best: Option<(Window, i64)> = None;
            for kid in kids {
                if let Some(kid_area) = area(xlib, display, *kid)
                    && kid_area * 2 > current_area
                    && best.is_none_or(|(_, best_area)| kid_area > best_area)
                {
                    best = Some((*kid, kid_area));
                }
            }
            (xlib.XFree)(children.cast());
            match best {
                Some((kid, kid_area)) => {
                    current = kid;
                    current_area = kid_area;
                }
                None => return (current != toplevel).then_some(current),
            }
        }
        (current != toplevel).then_some(current)
    }
}

/// A viewable window's pixel area, `None` when it is not mapped.
unsafe fn area(xlib: &Xlib, display: *mut Display, window: Window) -> Option<i64> {
    unsafe {
        let mut attributes = std::mem::MaybeUninit::<x11_dl::xlib::XWindowAttributes>::uninit();
        if (xlib.XGetWindowAttributes)(display, window, attributes.as_mut_ptr()) == 0 {
            return None;
        }
        let attributes = attributes.assume_init();
        if attributes.map_state != x11_dl::xlib::IsViewable {
            return None;
        }
        Some(i64::from(attributes.width) * i64::from(attributes.height))
    }
}

/// Whether `_NET_ACTIVE_WINDOW` names a window of this process.
fn active_window_is_ours() -> bool {
    let Ok(xlib) = Xlib::open() else { return false };
    // SAFETY: as in `focus_browser`.
    unsafe {
        let display = (xlib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            return false;
        }
        let mut ours = false;
        {
            let atom = (xlib.XInternAtom)(display, c"_NET_ACTIVE_WINDOW".as_ptr(), 0);
            let root = (xlib.XDefaultRootWindow)(display);
            let (mut actual_type, mut actual_format) = (0, 0);
            let (mut nitems, mut bytes_after) = (0u64, 0u64);
            let mut data: *mut u8 = std::ptr::null_mut();
            if atom != 0
                && (xlib.XGetWindowProperty)(
                    display,
                    root,
                    atom,
                    0,
                    1,
                    0,
                    x11_dl::xlib::XA_WINDOW,
                    &mut actual_type,
                    &mut actual_format,
                    &mut nitems,
                    &mut bytes_after,
                    &mut data,
                ) == 0
                && !data.is_null()
            {
                let active = *(data as *const Window);
                let pid_atom = (xlib.XInternAtom)(display, c"_NET_WM_PID".as_ptr(), 0);
                ours = window_pid(&xlib, display, active, pid_atom) == Some(std::process::id() as Pid);
                (xlib.XFree)(data.cast());
            }
            (xlib.XCloseDisplay)(display);
        }
        ours
    }
}

/// Re-applies the focus for a few seconds: Chromium's window appears a moment after the
/// GTK window is shown.
pub fn focus_browser_soon() {
    std::thread::spawn(|| {
        for _ in 0..60 {
            if focus_browser() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
}

/// Keeps the keyboard on Chromium's window for the life of the process.
///
/// GTK takes the X focus back whenever it activates the window (and whenever a GTK widget
/// asks for focus), which Tauri's own focus events do not report, so the focus is put back
/// on Chromium's window as long as this process owns the active window.
pub fn focus_browser_watchdog() {
    std::thread::Builder::new()
        .name("openclion-x11-focus".into())
        .spawn(|| {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if active_window_is_ours() {
                    focus_browser();
                }
            }
        })
        .ok();
}
