use smithay::{
    delegate_layer_shell, desktop::{WindowSurfaceType, layer_map_for_output}, output::Output, reexports::wayland_server::protocol::{wl_output::WlOutput, wl_surface::WlSurface}, utils::SERIAL_COUNTER, wayland::shell::wlr_layer::{KeyboardInteractivity, Layer, LayerSurface, LayerSurfaceConfigure, WlrLayerShellHandler}
};
use crate::AeroWM;

impl WlrLayerShellHandler for AeroWM {
    fn shell_state(&mut self) -> &mut smithay::wayland::shell::wlr_layer::WlrLayerShellState {
        &mut self.wlr_layer_shell_state
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        let Some(output) = self.space.outputs().find(|o| {
            layer_map_for_output(o)
                .layer_for_surface(surface.wl_surface(), WindowSurfaceType::TOPLEVEL)
                .is_some()
        }) else { return; };                                                                                
        let mut map = layer_map_for_output(&output);
        if let Some(_layer) = map.layer_for_surface(surface.wl_surface(), WindowSurfaceType::TOPLEVEL) {                                     
            let layer = map.layer_for_surface(surface.wl_surface(), WindowSurfaceType::TOPLEVEL).cloned();                                      
            if let Some(layer) = layer {                                                                                                        
                map.unmap_layer(&layer);                                                                                                        
            }                                                                                        
        }
        self.layer_surfaces.retain(|s| s.wl_surface() != surface.wl_surface());
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        // get the output from the parameter of the function, 
        // if thats None use the output under the cursor,
        // if thats None use the first output in the space
        let Some(output) = output.as_ref()
            .and_then(|o| Output::from_resource(o))
            .or_else(|| self.output_under_cursor().cloned())
            .or_else(|| self.space.outputs().next().cloned()) else { return; };
        let layer_surface = smithay::desktop::LayerSurface::new(surface.clone(), namespace);
        if let Err(e) = layer_map_for_output(&output).map_layer(&layer_surface) { 
            eprintln!("Failed to map new layer surface, returning with error: {}", e)
        }
        surface.send_configure();
        self.layer_surfaces.push(layer_surface);
    }

    fn ack_configure(&mut self, 
        surface: WlSurface, 
        _configure: LayerSurfaceConfigure
    ) {
        let Some(output) = self.space.outputs().find(|o| {
            layer_map_for_output(o)
                .layer_for_surface(&surface, WindowSurfaceType::TOPLEVEL)
                .is_some()
        }) else { return; };
        layer_map_for_output(&output).arrange();
        let Some(layer_surface) = self.layer_surfaces.iter().find(|s| s.wl_surface() == &surface) else { return; };
        if layer_surface.cached_state().keyboard_interactivity != KeyboardInteractivity::None {
            let keyboard = self.seat.get_keyboard().expect("Keyboard not found while trying to add it");
            let serial = SERIAL_COUNTER.next_serial();
            keyboard.set_focus(self, Some(layer_surface.wl_surface().clone()), serial);
        }
    }
}

delegate_layer_shell!(AeroWM);