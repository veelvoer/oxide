//! ext-session-lock (screen lockers) and idle notification / inhibition.
use std::collections::HashSet;

use smithay::{
    delegate_idle_inhibit, delegate_idle_notify, delegate_session_lock,
    reexports::wayland_server::protocol::{wl_output::WlOutput, wl_surface::WlSurface},
    output::Output,
    utils::SERIAL_COUNTER,
    wayland::{
        idle_inhibit::IdleInhibitHandler,
        idle_notify::{IdleNotifierHandler, IdleNotifierState},
        session_lock::{LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker},
    },
};
use tracing::info;

use crate::state::Oxide;

#[derive(Default)]
pub struct LockState {
    pub active: bool,
    pub surfaces: Vec<(Output, LockSurface)>,
}

impl SessionLockHandler for Oxide {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock_state
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        info!("session locked");
        // a locker that took over from one that died: its leftovers are gone
        self.lock.surfaces.clear();
        self.lock.active = true;
        self.dirty = true;
        // nothing of the session is drawn any more, so the lock is effective now
        confirmation.lock();
    }

    fn unlock(&mut self) {
        info!("session unlocked");
        self.lock = LockState::default();
        self.refocus();
        self.dirty = true;
    }

    fn new_surface(&mut self, surface: LockSurface, wl_output: WlOutput) {
        let Some(output) = Output::from_resource(&wl_output) else { return };
        if let Some(geo) = self.space.output_geometry(&output) {
            surface.with_pending_state(|s| s.size = Some((geo.size.w as u32, geo.size.h as u32).into()));
            surface.send_configure();
        }
        let wl = surface.wl_surface().clone();
        let _ = wl;
        info!("lock: new lock surface");
        self.lock.surfaces.push((output, surface));
        self.focus_lock();
        // the client may not have been ready for the first keyboard enter: send it again once it has drawn
        use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
        let _ = self.loop_handle.insert_source(Timer::from_duration(std::time::Duration::from_millis(500)), |_, _, state| {
            if state.locked() {
                if let Some(kb) = state.seat.get_keyboard() {
                    kb.set_focus(state, None, SERIAL_COUNTER.next_serial());
                }
                state.focus_lock();
            }
            TimeoutAction::Drop
        });
        self.dirty = true;
    }
}
delegate_session_lock!(Oxide);

impl IdleNotifierHandler for Oxide {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle_notifier_state
    }
}
delegate_idle_notify!(Oxide);

impl IdleInhibitHandler for Oxide {
    fn inhibit(&mut self, surface: WlSurface) {
        self.idle_inhibitors.insert(surface);
        self.idle_notifier_state.set_is_inhibited(true);
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        self.idle_inhibitors.remove(&surface);
        self.idle_notifier_state.set_is_inhibited(!self.idle_inhibitors.is_empty());
    }
}
delegate_idle_inhibit!(Oxide);

impl Oxide {
    pub fn locked(&self) -> bool {
        self.lock.active
    }

    /// While locked the keyboard belongs to a lock surface, whatever else asks for focus.
    pub fn focus_lock(&mut self) {
        let want = self
            .monitors
            .get(self.focused_mon)
            .and_then(|m| self.lock_surface_for(&m.output))
            .or_else(|| self.lock.surfaces.first().map(|(_, s)| s))
            .map(|s| s.wl_surface().clone());
        if let Some(wl) = want {
            let kb = self.seat.get_keyboard().unwrap();
            if kb.current_focus().as_ref() != Some(&wl) {
                info!("lock: keyboard focus to the lock surface");
                kb.set_focus(self, Some(wl), SERIAL_COUNTER.next_serial());
            }
        }
    }

    pub fn lock_surface_for(&self, output: &Output) -> Option<&LockSurface> {
        self.lock.surfaces.iter().find(|(o, _)| o == output).map(|(_, s)| s)
    }
}

#[allow(dead_code)]
fn _types(_: HashSet<WlSurface>) {}
