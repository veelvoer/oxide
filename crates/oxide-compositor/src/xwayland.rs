//! XWayland support: an X11 window manager driven by the tiling logic.
use std::{os::unix::io::OwnedFd, process::Stdio};

use smithay::{
    delegate_xwayland_shell,
    desktop::Window,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Rectangle},
    wayland::{
        selection::{
            data_device::{
                clear_data_device_selection, current_data_device_selection_userdata,
                request_data_device_client_selection, set_data_device_selection,
            },
            primary_selection::{
                clear_primary_selection, current_primary_selection_userdata, request_primary_client_selection,
                set_primary_selection,
            },
            SelectionTarget,
        },
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        xwm::{Reorder, ResizeEdge, XwmId},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
    },
};
use tracing::{error, info, warn};

use crate::state::Oxide;

impl XWaylandShellHandler for Oxide {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }

    fn surface_associated(&mut self, _xwm: XwmId, _wl_surface: WlSurface, surface: X11Surface) {
        // the keyboard focus needs the wl_surface, which only exists from now on
        if self.focus.as_ref().and_then(|w| w.x11_surface()).is_some_and(|x| *x == surface) {
            let w = self.focus.clone();
            self.focus_window(w);
        }
    }
}
delegate_xwayland_shell!(Oxide);

impl Oxide {
    pub fn start_xwayland(&mut self) {
        let (xwayland, client) = match XWayland::spawn(
            &self.display_handle,
            None,
            std::env::var("LD_LIBRARY_PATH").ok().map(|v| ("LD_LIBRARY_PATH".to_string(), v)),
            true,
            Stdio::null(),
            Stdio::inherit(),
            |_| (),
        ) {
            Ok(x) => x,
            Err(e) => return warn!("XWayland not started: {e}"),
        };
        let res = self.loop_handle.insert_source(xwayland, move |event, _, state| match event {
            XWaylandEvent::Ready { x11_socket, display_number } => {
                match X11Wm::start_wm(state.loop_handle.clone(), x11_socket, client.clone()) {
                    Ok(mut wm) => {
                        let img = state.cursor.get("default", 1, std::time::Duration::ZERO);
                        let _ = img;
                        let _ = wm.set_cursor(&[0; 4], (1, 1).into(), (0, 0).into());
                        state.xwm = Some(wm);
                        std::env::set_var("DISPLAY", format!(":{display_number}"));
                        info!("XWayland ready on :{display_number}");
                        // dbus-activated programs and services find the X display too, then the shell and autostart go
                        state.spawn("command -v dbus-update-activation-environment >/dev/null && dbus-update-activation-environment --systemd DISPLAY");
                        state.finish_startup();
                    }
                    Err(e) => error!("cannot attach X11 window manager: {e}"),
                }
            }
            XWaylandEvent::Error => {
                warn!("XWayland crashed on startup");
                state.finish_startup();
            }
        });
        if let Err(e) = res {
            error!("cannot insert XWayland source: {e}");
        }
    }

    fn x11_win_idx(&self, window: &X11Surface) -> Option<usize> {
        self.wins.iter().position(|w| w.window.x11_surface().is_some_and(|x| x == window))
    }
}

impl XwmHandler for Oxide {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().expect("xwm not started")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if window.set_mapped(true).is_err() {
            return;
        }
        self.map_pending(Window::new_x11_window(window));
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        // menus and tooltips place themselves
        let loc = window.geometry().loc;
        let w = Window::new_x11_window(window);
        self.space.map_element(w.clone(), loc, true);
        self.or_windows.push(w);
        self.dirty = true;
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.forget_x11(&window);
        if !window.is_override_redirect() {
            let _ = window.set_mapped(false);
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.forget_x11(&window);
    }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        if self.x11_win_idx(&window).is_some() {
            // managed windows are laid out by us; repeat the current geometry
            let _ = window.configure(window.geometry());
            return;
        }
        let mut geo = window.geometry();
        if let (Some(x), Some(y)) = (x, y) {
            geo.loc = (x, y).into();
        }
        if let Some(w) = w {
            geo.size.w = w as i32;
        }
        if let Some(h) = h {
            geo.size.h = h as i32;
        }
        let _ = window.configure(geo);
    }

    fn configure_notify(&mut self, _xwm: XwmId, window: X11Surface, geometry: Rectangle<i32, Logical>, _above: Option<u32>) {
        // override-redirect windows move themselves
        if let Some(w) = self.or_windows.iter().find(|w| w.x11_surface().is_some_and(|x| *x == window)).cloned() {
            self.space.map_element(w, geometry.loc, false);
            self.dirty = true;
        }
    }

    fn maximize_request(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn unmaximize_request(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(i) = self.x11_win_idx(&window) {
            self.wins[i].fullscreen = true;
            self.arrange();
        }
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(i) = self.x11_win_idx(&window) {
            self.wins[i].fullscreen = false;
            self.arrange();
        }
    }

    fn resize_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32, _edges: ResizeEdge) {}
    fn move_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32) {}

    fn allow_selection_access(&mut self, xwm: XwmId, _selection: SelectionTarget) -> bool {
        self.focus
            .as_ref()
            .and_then(|w| w.x11_surface())
            .is_some_and(|x| x.xwm_id() == Some(xwm))
    }

    fn send_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_type: String, fd: OwnedFd) {
        match selection {
            SelectionTarget::Clipboard => {
                if let Err(e) = request_data_device_client_selection(&self.seat, mime_type, fd) {
                    error!(?e, "failed to forward clipboard to XWayland");
                }
            }
            SelectionTarget::Primary => {
                if let Err(e) = request_primary_client_selection(&self.seat, mime_type, fd) {
                    error!(?e, "failed to forward primary selection to XWayland");
                }
            }
        }
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        match selection {
            SelectionTarget::Clipboard => set_data_device_selection(&self.display_handle, &self.seat, mime_types, ()),
            SelectionTarget::Primary => set_primary_selection(&self.display_handle, &self.seat, mime_types, ()),
        }
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        match selection {
            SelectionTarget::Clipboard => {
                if current_data_device_selection_userdata(&self.seat).is_some() {
                    clear_data_device_selection(&self.display_handle, &self.seat)
                }
            }
            SelectionTarget::Primary => {
                if current_primary_selection_userdata(&self.seat).is_some() {
                    clear_primary_selection(&self.display_handle, &self.seat)
                }
            }
        }
    }
}

impl Oxide {
    fn forget_x11(&mut self, window: &X11Surface) {
        if let Some(pos) = self.or_windows.iter().position(|w| w.x11_surface().is_some_and(|x| x == window)) {
            let w = self.or_windows.remove(pos);
            self.space.unmap_elem(&w);
            self.dirty = true;
        }
        if let Some(i) = self.x11_win_idx(window) {
            self.remove_win_at(i);
        }
    }
}
