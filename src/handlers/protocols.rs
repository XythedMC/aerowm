use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface, 
    wayland::{
        idle_inhibit::{IdleInhibitHandler, IdleInhibitManagerState}, idle_notify::{IdleNotifierHandler, IdleNotifierState}
    },
    delegate_idle_notify,
    delegate_idle_inhibit,
};

use crate::AeroWM;

impl IdleNotifierHandler for AeroWM {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle_notifier_state
    }
}

impl IdleInhibitHandler for AeroWM {
    fn inhibit(&mut self, _surface: WlSurface) {
        self.idle_notifier_state.set_is_inhibited(true);
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        self.idle_notifier_state.set_is_inhibited(false);
    }
}

delegate_idle_notify!(AeroWM);
delegate_idle_inhibit!(AeroWM);


