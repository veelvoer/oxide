//! Wayland protocol handlers.
use std::os::unix::io::OwnedFd;

use smithay::{
    backend::{
        allocator::dmabuf::Dmabuf,
        renderer::{
            utils::on_commit_buffer_handler, ImportDma,
        },
    },
    delegate_compositor, delegate_data_device, delegate_dmabuf, delegate_layer_shell, delegate_output,
    delegate_cursor_shape, delegate_data_control, delegate_fractional_scale, delegate_tablet_manager, delegate_primary_selection, delegate_seat, delegate_xdg_activation, delegate_shm, delegate_viewporter, delegate_xdg_decoration,
    delegate_xdg_shell,
    desktop::{
        find_popup_root_surface, layer_map_for_output, LayerSurface, PopupKeyboardGrab, PopupKind,
        PopupPointerGrab, PopupUngrabStrategy, Window, WindowSurfaceType,
    },
    input::{pointer::Focus, Seat, SeatHandler, SeatState},
    output::Output,
    reexports::{
        wayland_protocols::xdg::{
            decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecoMode,
            shell::server::xdg_toplevel::State as TState,
        },
        wayland_server::{
            protocol::{wl_buffer, wl_output, wl_seat, wl_surface::WlSurface},
            Client, Resource,
        },
    },
    utils::Serial,
    wayland::{
        buffer::BufferHandler,
        compositor::{
            get_parent, is_sync_subsurface, with_states, CompositorClientState, CompositorHandler,
            CompositorState,
        },
        dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
        output::OutputHandler,
        selection::{
            data_device::{
                set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState,
                ServerDndGrabHandler,
            },
            primary_selection::{set_primary_focus, PrimarySelectionHandler, PrimarySelectionState},
            wlr_data_control::{DataControlHandler, DataControlState},
            SelectionHandler,
        },
        shell::{
            wlr_layer::{
                Layer, LayerSurfaceData, WlrLayerShellHandler, WlrLayerShellState, KeyboardInteractivity,
                LayerSurfaceCachedState,
            },
            xdg::{
                decoration::XdgDecorationHandler, PopupSurface, PositionerState, ToplevelSurface,
                XdgShellHandler, XdgShellState,
            },
        },
        shm::{ShmHandler, ShmState},
        pointer_constraints::{with_pointer_constraint, PointerConstraintsHandler},
        fractional_scale::{with_fractional_scale, FractionalScaleHandler},
        tablet_manager::TabletSeatHandler,
        xdg_activation::{XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData},
    },
};
use tracing::warn;

use crate::state::{ClientState, Oxide};

impl BufferHandler for Oxide {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl CompositorHandler for Oxide {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        if let Some(state) = client.get_data::<smithay::xwayland::XWaylandClientData>() {
            return &state.compositor_state;
        }
        &client.get_data::<ClientState>().expect("unknown client data").compositor_state
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        use smithay::{
            wayland::{
                compositor::{add_blocker, add_pre_commit_hook, BufferAssignment, SurfaceAttributes},
                dmabuf::get_dmabuf,
            },
        };
        // Hold back commits whose dmabuf is not ready yet.
        add_pre_commit_hook::<Self, _>(surface, move |state, _dh, surface| {
            let dmabuf = with_states(surface, |data| {
                data.cached_state.get::<SurfaceAttributes>().pending().buffer.as_ref().and_then(
                    |assignment| match assignment {
                        BufferAssignment::NewBuffer(buffer) => get_dmabuf(buffer).cloned().ok(),
                        _ => None,
                    },
                )
            });
            if let Some(dmabuf) = dmabuf {
                if let Ok((blocker, source)) = dmabuf.generate_blocker(smithay::reexports::calloop::Interest::READ) {
                    if let Some(client) = surface.client() {
                        let res = state.loop_handle.insert_source(source, move |_, _, data| {
                            let dh = data.display_handle.clone();
                            data.client_compositor_state(&client).blocker_cleared(data, &dh);
                            Ok(())
                        });
                        if res.is_ok() {
                            add_blocker(surface, blocker);
                        }
                    }
                }
            }
        });
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);

        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self.window_for_surface(&root) {
                window.on_commit();
            }
            if let Some(i) = self.wins.iter().position(|w| w.held.is_some() && w.window.toplevel().is_some_and(|t| t.wl_surface() == &root)) {
                self.held_commit(i);
            }
            if let Some(i) = self.wins.iter().position(|w| w.auto_float && w.window.toplevel().is_some_and(|t| t.wl_surface() == &root)) {
                if self.wins[i].floating {
                    self.float_to_client_size(i);
                } else {
                    self.wins[i].auto_float = false;
                }
            }
        }
        self.popups.commit(surface);

        // first commit of a toplevel: now app_id/title are known, place it
        if let Some(pos) = self.pending.iter().position(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface)) {
            let window = self.pending.remove(pos);
            self.map_pending(window);
        }

        if let Some(PopupKind::Xdg(popup)) = self.popups.find_popup(surface) {
            if !popup.is_initial_configure_sent() {
                let _ = popup.send_configure();
            }
        }

        self.layer_commit(surface);
        self.dirty = true;
    }
}

impl Oxide {
    fn layer_commit(&mut self, surface: &WlSurface) {
        let Some(output) = self
            .monitors
            .iter()
            .map(|m| m.output.clone())
            .find(|o| layer_map_for_output(o).layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).is_some())
        else {
            return;
        };
        let initial_sent = with_states(surface, |s| {
            s.data_map.get::<LayerSurfaceData>().unwrap().lock().unwrap().initial_configure_sent
        });
        let layer = {
            let mut map = layer_map_for_output(&output);
            map.arrange();
            map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).cloned()
        };
        let Some(layer) = layer else { return };
        if !initial_sent {
            layer.layer_surface().send_configure();
        }
        // exclusive zones may have changed
        self.arrange();
        // launchers and lock screens ask for keyboard focus
        let (interactivity, lyr) = with_states(surface, |s| {
            let st = *s.cached_state.get::<LayerSurfaceCachedState>().current();
            (st.keyboard_interactivity, st.layer)
        });
        if lyr == Layer::Background {
            let now = std::time::Instant::now();
            self.bg_dirty = Some((self.bg_dirty.map_or(now, |d| d.0), now));
        }
        // a layer asks for focus when it starts wanting the keyboard (on map, or later when a hidden
        // popup is shown again) and gives it back when it stops
        let wants = interactivity != KeyboardInteractivity::None && matches!(lyr, Layer::Top | Layer::Overlay);
        let had = self.kb_layers.contains(surface);
        if wants && !had {
            self.kb_layers.push(surface.clone());
            self.focus_layer(surface.clone());
        } else if !wants && had {
            self.kb_layers.retain(|s| s != surface);
            if self.layer_focus.as_ref() == Some(surface) {
                self.layer_focus = None;
                self.refocus();
            }
        }
    }
}

impl ShmHandler for Oxide {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl XdgShellHandler for Oxide {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        // placed (and configured) on first commit, see CompositorHandler::commit
        self.pending.push(Window::new_wayland_window(surface));
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.start_close(surface.wl_surface());
        self.remove_window(surface.wl_surface());
        self.dirty = true;
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        if let Err(e) = self.popups.track_popup(PopupKind::Xdg(surface)) {
            warn!("error tracking popup: {e}");
        }
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn grab(&mut self, surface: PopupSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let seat: Seat<Oxide> = Seat::from_resource(&seat).unwrap();
        let kind = PopupKind::Xdg(surface);
        let Ok(root) = find_popup_root_surface(&kind) else { return };
        if let Ok(mut grab) = self.popups.grab_popup(root, kind, &seat, serial) {
            if let Some(keyboard) = seat.get_keyboard() {
                if keyboard.is_grabbed()
                    && !(keyboard.has_grab(serial) || keyboard.has_grab(grab.previous_serial().unwrap_or(serial)))
                {
                    grab.ungrab(PopupUngrabStrategy::All);
                    return;
                }
                keyboard.set_focus(self, grab.current_grab(), serial);
                keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
            }
            if let Some(pointer) = seat.get_pointer() {
                if pointer.is_grabbed()
                    && !(pointer.has_grab(serial)
                        || pointer.has_grab(grab.previous_serial().unwrap_or_else(|| grab.serial())))
                {
                    grab.ungrab(PopupUngrabStrategy::All);
                    return;
                }
                pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
            }
        }
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<wl_output::WlOutput>) {
        if let Some(i) = self.window_for_surface(surface.wl_surface()).and_then(|w| self.win_idx(&w)) {
            self.wins[i].fullscreen = true;
            self.arrange();
        }
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        if let Some(i) = self.window_for_surface(surface.wl_surface()).and_then(|w| self.win_idx(&w)) {
            self.wins[i].fullscreen = false;
            self.arrange();
        }
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        // tiling: ignore, but answer with a configure so the client does not hang
        surface.send_pending_configure();
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        surface.send_pending_configure();
    }
}

impl Oxide {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else { return };
        let Some(window) = self.window_for_surface(&root) else { return };
        let Some(out) = self.monitors.get(self.focused_mon).map(|m| m.output.clone()) else { return };
        let Some(out_geo) = self.space.output_geometry(&out) else { return };
        let Some(win_geo) = self.space.element_geometry(&window) else { return };
        let mut target = out_geo;
        target.loc -= smithay::desktop::get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= win_geo.loc;
        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}

impl XdgDecorationHandler for Oxide {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|s| s.decoration_mode = Some(DecoMode::ServerSide));
    }
    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: DecoMode) {
        // the compositor draws borders; never ask clients to draw their own
        toplevel.with_pending_state(|s| s.decoration_mode = Some(DecoMode::ServerSide));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }
    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|s| s.decoration_mode = Some(DecoMode::ServerSide));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }
}

impl WlrLayerShellHandler for Oxide {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: smithay::wayland::shell::wlr_layer::LayerSurface,
        wl_output: Option<wl_output::WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output = wl_output
            .as_ref()
            .and_then(Output::from_resource)
            .or_else(|| self.monitors.get(self.focused_mon).map(|m| m.output.clone()));
        let Some(output) = output else { return };
        let mut map = layer_map_for_output(&output);
        if let Err(e) = map.map_layer(&LayerSurface::new(surface, namespace)) {
            warn!("cannot map layer surface: {e}");
        }
    }

    fn layer_destroyed(&mut self, surface: smithay::wayland::shell::wlr_layer::LayerSurface) {
        let had_focus = self.layer_focus.as_ref() == Some(surface.wl_surface());
        self.kb_layers.retain(|s| s != surface.wl_surface());
        for m in &self.monitors {
            let mut map = layer_map_for_output(&m.output);
            let layer = map.layers().find(|l| l.layer_surface() == &surface).cloned();
            if let Some(layer) = layer {
                map.unmap_layer(&layer);
            }
        }
        if had_focus {
            self.layer_focus = None;
            self.refocus();
        }
        self.arrange();
    }
}

impl SeatHandler for Oxide {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Oxide> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client.clone());
        set_primary_focus(dh, seat, client);
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: smithay::input::pointer::CursorImageStatus) {
        // X11 menus and dropdowns (override-redirect windows) have no cursor of their own: XWayland then asks for none
        // and the pointer would vanish over them. The normal arrow is what the user expects there.
        if matches!(image, smithay::input::pointer::CursorImageStatus::Hidden) {
            use smithay::{wayland::seat::WaylandFocus, xwayland::xwm::WmWindowType as T};
            let popup_like = |w: &smithay::desktop::Window| {
                w.x11_surface().is_some_and(|x| matches!(x.window_type(), Some(T::DropdownMenu | T::Menu | T::PopupMenu | T::Tooltip | T::Notification)))
            };
            let over_popup = self.pointer.current_focus().is_some_and(|f| {
                self.or_windows.iter().any(|w| w.wl_surface().is_some_and(|s| *s == f))
                    || self.wins.iter().any(|w| popup_like(&w.window) && w.window.wl_surface().is_some_and(|s| *s == f))
            });
            if over_popup {
                self.cursor_status = smithay::input::pointer::CursorImageStatus::default_named();
                return;
            }
        }
        self.cursor_status = image;
    }
}

impl SelectionHandler for Oxide {
    type SelectionUserData = ();
}

impl DataDeviceHandler for Oxide {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
impl ClientDndGrabHandler for Oxide {}
impl ServerDndGrabHandler for Oxide {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {}
}

impl PrimarySelectionHandler for Oxide {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}

impl DataControlHandler for Oxide {
    fn data_control_state(&self) -> &DataControlState {
        &self.data_control_state
    }
}
delegate_data_control!(Oxide);

impl OutputHandler for Oxide {}

impl DmabufHandler for Oxide {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(&mut self, _global: &DmabufGlobal, dmabuf: Dmabuf, notifier: ImportNotifier) {
        let ok = self.renderer_mut().is_some_and(|r| r.import_dmabuf(&dmabuf, None).is_ok());
        if ok {
            let _ = notifier.successful::<Oxide>();
        } else {
            notifier.failed();
        }
    }
}

impl FractionalScaleHandler for Oxide {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        let scale = self
            .monitors
            .get(self.focused_mon)
            .map_or(1.0, |m| m.output.current_scale().fractional_scale());
        with_states(&surface, |states| {
            with_fractional_scale(states, |fs| fs.set_preferred_scale(scale));
        });
    }
}

impl XdgActivationHandler for Oxide {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.xdg_activation_state
    }

    fn request_activation(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData, surface: WlSurface) {
        // only honour tokens created recently, e.g. by a launcher that just ran the app
        if data.timestamp.elapsed().as_secs() < 10 {
            if let Some(w) = self.window_for_surface(&surface) {
                self.focus_window(Some(w));
            }
        }
    }
}

impl PointerConstraintsHandler for Oxide {
    fn new_constraint(&mut self, surface: &WlSurface, pointer: &smithay::input::pointer::PointerHandle<Self>) {
        if pointer.current_focus().as_ref() == Some(surface) {
            with_pointer_constraint(surface, pointer, |c| {
                if let Some(c) = c {
                    c.activate();
                }
            });
            self.constraint_surface = Some(surface.clone());
        }
    }

    fn cursor_position_hint(
        &mut self,
        _surface: &WlSurface,
        _pointer: &smithay::input::pointer::PointerHandle<Self>,
        _location: smithay::utils::Point<f64, smithay::utils::Logical>,
    ) {
    }
}
smithay::delegate_pointer_constraints!(Oxide);
smithay::delegate_relative_pointer!(Oxide);

delegate_compositor!(Oxide);
delegate_fractional_scale!(Oxide);
delegate_xdg_activation!(Oxide);
delegate_cursor_shape!(Oxide);

impl TabletSeatHandler for Oxide {
    fn tablet_tool_image(&mut self, _tool: &smithay::backend::input::TabletToolDescriptor, image: smithay::input::pointer::CursorImageStatus) {
        self.cursor_status = image;
    }
}
delegate_tablet_manager!(Oxide);
delegate_shm!(Oxide);
delegate_xdg_shell!(Oxide);
delegate_xdg_decoration!(Oxide);
delegate_layer_shell!(Oxide);
delegate_seat!(Oxide);
delegate_data_device!(Oxide);
delegate_primary_selection!(Oxide);
delegate_output!(Oxide);
delegate_dmabuf!(Oxide);
delegate_viewporter!(Oxide);

#[allow(dead_code)]
fn _unused(_: TState) {}
