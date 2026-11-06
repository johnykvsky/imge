use raw_window_handle::RawWindowHandle;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
    WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

/// Reads and removes any Freedesktop startup notification token or XDG activation token
/// from the process environment so child processes do not inherit them.
pub fn take_startup_id() -> Option<String> {
    let id = std::env::var("DESKTOP_STARTUP_ID")
        .or_else(|_| std::env::var("XDG_ACTIVATION_TOKEN"))
        .ok()
        .filter(|s| !s.trim().is_empty());

    // Clean up environment variables according to the Freedesktop specification.
    // In Rust 2024 edition, environment variable modification is unsafe in multithreaded contexts.
    // Here it is called once at the start of main() before additional threads are spawned.
    unsafe {
        std::env::remove_var("DESKTOP_STARTUP_ID");
        std::env::remove_var("XDG_ACTIVATION_TOKEN");
    }

    id
}

/// Extracts the user interaction timestamp (in ms) from a Freedesktop startup ID.
/// Typically formatted as `..._TIME<timestamp>`.
fn extract_timestamp(startup_id: &str) -> Option<u32> {
    if let Some(pos) = startup_id.rfind("_TIME") {
        let rest = &startup_id[pos + 5..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse::<u32>().ok()
    } else {
        None
    }
}

/// Completes startup notification and ensures window activation / focus.
///
/// On X11:
/// - Sets `_NET_STARTUP_ID` on the window.
/// - Sets `_NET_WM_USER_TIME` if a launch timestamp was provided.
/// - Sends the Freedesktop `remove: ID="..."` message to the root window to immediately
///   dismiss the desktop launcher's spinning cursor.
/// - Sends `_NET_ACTIVE_WINDOW` with source=2 (pager/launcher) to cleanly grant focus
///   under compositors enforcing Focus Stealing Prevention (FSP) like Cinnamon (Muffin) and GNOME (Mutter).
pub fn complete_startup(
    raw_handle: Option<RawWindowHandle>,
    startup_id: Option<&str>,
) {
    let Some(raw_handle) = raw_handle else { return };

    let window_id: u32 = match raw_handle {
        RawWindowHandle::Xlib(h) => h.window as u32,
        RawWindowHandle::Xcb(h) => h.window.get(),
        _ => return, // Wayland or other platforms
    };

    let (conn, screen_num) = match x11rb::connect(None) {
        Ok(res) => res,
        Err(_) => return,
    };

    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // Intern required EWMH and Startup Notification atoms
    let net_startup_id = conn
        .intern_atom(false, b"_NET_STARTUP_ID")
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom);

    let net_startup_info_begin = conn
        .intern_atom(false, b"_NET_STARTUP_INFO_BEGIN")
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom);

    let net_startup_info = conn
        .intern_atom(false, b"_NET_STARTUP_INFO")
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom);

    let net_active_window = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom);

    let net_user_time = conn
        .intern_atom(false, b"_NET_WM_USER_TIME")
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom);

    let timestamp = startup_id.and_then(extract_timestamp);

    // 1. If we have a startup_id, set _NET_STARTUP_ID on our window
    if let (Some(id), Some(atom)) = (startup_id, net_startup_id) {
        let _ = conn.change_property8(
            PropMode::REPLACE,
            window_id,
            atom,
            AtomEnum::STRING,
            id.as_bytes(),
        );
    }

    // 2. Set _NET_WM_USER_TIME to let the window manager match the user launch action
    if let (Some(ts), Some(atom)) = (timestamp, net_user_time) {
        let _ = conn.change_property32(
            PropMode::REPLACE,
            window_id,
            atom,
            AtomEnum::CARDINAL,
            &[ts],
        );
    }

    // 3. Complete the startup notification sequence by sending remove: ID="..."
    if let (Some(id), Some(info_begin), Some(info_cont)) =
        (startup_id, net_startup_info_begin, net_startup_info)
    {
        let escaped_id = id.replace('\\', "\\\\").replace('"', "\\\"");
        let message = format!("remove: ID=\"{escaped_id}\"");
        let bytes = message.as_bytes();

        // Create an unmapped 1x1 dummy window to act as message sender
        let dummy = conn.generate_id().unwrap_or(window_id);
        let _ = conn.create_window(
            screen.root_depth,
            dummy,
            root,
            -100,
            -100,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &CreateWindowAux::new().override_redirect(1),
        );

        let mut msg_type = info_begin;
        for chunk in bytes.chunks(20) {
            let mut data = [0u8; 20];
            data[..chunk.len()].copy_from_slice(chunk);
            let event = ClientMessageEvent::new(8, dummy, msg_type, data);
            let _ = conn.send_event(false, root, EventMask::PROPERTY_CHANGE, event);
            msg_type = info_cont;
        }

        let _ = conn.destroy_window(dummy);
    }

    // 4. Request window activation via _NET_ACTIVE_WINDOW
    // Source 2 = pager / launcher bypasses compositor FSP without external xdotool
    if let Some(atom) = net_active_window {
        let ts = timestamp.unwrap_or(0);
        let event = ClientMessageEvent::new(32, window_id, atom, [2, ts, 0, 0, 0]);
        let _ = conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
    }

    let _ = conn.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_timestamp() {
        assert_eq!(extract_timestamp("nemo-1234_TIME98765"), Some(98765));
        assert_eq!(extract_timestamp("simple_id"), None);
    }

    #[test]
    fn test_x11rb_connect() {
        if std::env::var("DISPLAY").is_ok() {
            let res = x11rb::connect(None);
            assert!(res.is_ok(), "x11rb::connect failed: {:?}", res.err());
            let (conn, screen_num) = res.unwrap();
            let screen = &conn.setup().roots[screen_num];
            let root = screen.root;
            let net_startup_id = conn
                .intern_atom(false, b"_NET_STARTUP_ID")
                .unwrap()
                .reply()
                .unwrap()
                .atom;
            let cookie = conn.change_property8(
                PropMode::REPLACE,
                root,
                net_startup_id,
                AtomEnum::STRING,
                b"test_prop",
            );
            assert!(cookie.is_ok(), "change_property8 failed: {:?}", cookie.err());
            let checked = cookie.unwrap().check();
            assert!(checked.is_ok(), "checked failed: {:?}", checked.err());
            conn.flush().unwrap();
        }
    }
}
