use smithay::{delegate_tablet_manager, wayland::tablet_manager::TabletSeatHandler};
use crate::AeroWM;

impl TabletSeatHandler for AeroWM {
    
}
delegate_tablet_manager!(AeroWM);