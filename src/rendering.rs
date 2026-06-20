use std::collections::HashMap;

use smithay::{
    backend::renderer::{
            Color32F, element::{AsRenderElements, Kind, solid::{SolidColorBuffer, SolidColorRenderElement}, surface::WaylandSurfaceRenderElement, texture::{TextureBuffer, TextureRenderElement}, utils::RescaleRenderElement}, gles::{
                GlesPixelProgram, GlesRenderer, GlesTexture, Uniform, UniformName, UniformType, element::PixelShaderElement
            }
        }, desktop::{Space, Window}, utils::{Logical, Point, Rectangle, Scale, Size, Transform}
};

use crate::{handlers::config::AeroWMConfig, state::{CanvasWindow, AeroWMElement, ViewMode}};

pub const LINE_FRAG: &str = r#"
precision highp float;
varying vec2 v_coords;
uniform vec2  p_start;
uniform vec2  p_end;
uniform float thickness;
uniform vec4  u_color;
uniform vec2  elem_size;

float dist_segment(vec2 p, vec2 a, vec2 b) {
    vec2  ab = b - a;
    float t  = clamp(dot(p - a, ab) / dot(ab, ab), 0.0, 1.0);
    return length(p - a - t * ab);
}

void main() {
    vec2  px = v_coords * elem_size;
    float d  = dist_segment(px, p_start, p_end);

    // Filled circles at endpoints (radius 4 px).
    float dot_r = 4.0;
    d = min(d, max(length(px - p_start) - dot_r, 0.0));
    d = min(d, max(length(px - p_end)   - dot_r, 0.0));

    float ht = thickness * 0.5;
    float a  = 1.0 - smoothstep(ht - 1.0, ht + 1.5, d);
    float fa = u_color.a * a;
    // Premultiplied alpha: transparent pixels must output (0,0,0,0), not (r,g,b,0).
    gl_FragColor = vec4(u_color.rgb * fa, fa);
}
"#;

/// Solid-color rectangle — used for the mode indicator square (premultiplied).
pub const SOLID_FRAG: &str = r#"
precision mediump float;
varying vec2 v_coords;
uniform vec4 u_color;
void main() {
    gl_FragColor = vec4(u_color.rgb * u_color.a, u_color.a);
}
"#;

pub const BORDER_FRAG: &str = r#"
precision highp float;
varying vec2 v_coords;
uniform vec2 elem_size;
uniform float radius;
uniform vec4 u_color;
uniform float thickness;

void main() {
    vec2 px = v_coords * elem_size;
    vec2 p = px - elem_size / 2.0;

    vec2 d = abs(p) - elem_size / 2.0 + vec2(radius);
    float dist = length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - radius;
    if (dist > 0.0) { discard; }
    if (dist < -thickness) { discard; }
    gl_FragColor = vec4(u_color.rgb * u_color.a, u_color.a);
}   
"#;

pub fn convert_color(color: [u8; 4]) -> [f32; 4] {
    let [r, g, b, a] = color;
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0]
}
// ── Shader compilation ─────────────────────────────────────────────────────────

pub fn compile_line(r: &mut GlesRenderer) -> Option<GlesPixelProgram> {
    r.compile_custom_pixel_shader(
        LINE_FRAG,
        &[
            UniformName::new("p_start",   UniformType::_2f),
            UniformName::new("p_end",     UniformType::_2f),
            UniformName::new("thickness", UniformType::_1f),
            UniformName::new("u_color",   UniformType::_4f),
            UniformName::new("elem_size", UniformType::_2f),
        ],
    )
    .map_err(|e| eprintln!("AeroWM: line shader compile failed: {e}"))
    .ok()
}

pub fn compile_solid(r: &mut GlesRenderer) -> Option<GlesPixelProgram> {
    r.compile_custom_pixel_shader(
        SOLID_FRAG,
        &[UniformName::new("u_color", UniformType::_4f)],
    )
    .map_err(|e| eprintln!("AeroWM: solid shader compile failed: {e}"))
    .ok()
}

pub fn compile_border(r: &mut GlesRenderer) -> Option<GlesPixelProgram> {
        r.compile_custom_pixel_shader(
        BORDER_FRAG,
        &[
            UniformName::new("elem_size", UniformType::_2f),
            UniformName::new("radius", UniformType::_1f),
            UniformName::new("thickness", UniformType::_1f),
            UniformName::new("u_color", UniformType::_4f),
        ],
    )
    .map_err(|e| eprintln!("AeroWM: border shader compile failed: {e}"))
    .ok()
}

// ── Per-frame element builders ─────────────────────────────────────────────────

pub fn line_element(prog: &GlesPixelProgram, start: (f32, f32), end: (f32, f32)) -> PixelShaderElement {
    let pad   = 8.0_f32;
    let min_x = start.0.min(end.0) - pad;
    let min_y = start.1.min(end.1) - pad;
    let max_x = start.0.max(end.0) + pad;
    let max_y = start.1.max(end.1) + pad;
    let ew    = (max_x - min_x).max(1.0);
    let eh    = (max_y - min_y).max(1.0);

    // Convert to element-local pixel coordinates.
    let ls = (start.0 - min_x, start.1 - min_y);
    let le = (end.0   - min_x, end.1   - min_y);

    let area = Rectangle {
        loc: (min_x as i32, min_y as i32).into(),
        size: (ew as i32, eh as i32).into(),
    };

    PixelShaderElement::new(
        prog.clone(),
        area,
        None,
        1.0,
        vec![
            Uniform::new("p_start",   (ls.0, ls.1)),
            Uniform::new("p_end",     (le.0, le.1)),
            Uniform::new("thickness", 2.0_f32),
            Uniform::new("u_color",   (0.55_f32, 0.78_f32, 1.0_f32, 0.45_f32)),
            Uniform::new("elem_size", (ew, eh)),
        ],
        Kind::Unspecified,
    )
}

pub fn connector_elements(windows: &[CanvasWindow], zoom: f64, viewport_x: f64, viewport_y: f64, prog: &GlesPixelProgram) -> Vec<PixelShaderElement> {
    windows
        .iter()
        .filter_map(|cw| {
            if cw.is_fullscreen { return None; }
            if cw.is_scratchpad { return None; }
            let pid    = cw.parent_id?;
            let parent = windows.iter().find(|p| p.id == pid)?;
            
            if parent.is_scratchpad { return None; }
            
            let z   = zoom as f32;
            let px = ((parent.canvas_x - viewport_x) * zoom) as f32;
            let py = ((parent.canvas_y - viewport_y) * zoom) as f32;
            let cx = ((cw.canvas_x    - viewport_x) * zoom) as f32;
            let cy = ((cw.canvas_y    - viewport_y) * zoom) as f32;

            let phw = parent.base_width  as f32 * z / 2.0;
            let ph  = parent.base_height as f32 * z;
            let chw = cw.base_width      as f32 * z / 2.0;

            // Parent bottom-center → child top-center.
            Some(line_element(prog, (px + phw, py + ph), (cx + chw, cy)))
        })
        .collect()
}

pub fn focus_border_elements(
    focused_window_id: Option<u32>,
    config: AeroWMConfig,
    zoom: f64,
    prog: &GlesPixelProgram, 
    cw: &CanvasWindow, 
    geo: Rectangle<i32, Logical>
) -> PixelShaderElement {
    let fid = focused_window_id;
    let bw = config.border_width as i32;
    let sx = geo.loc.x - bw;
    let sy = geo.loc.y - bw;
    let ww = (geo.size.w as f64 * zoom) as i32 + bw * 2;
    let wh = (geo.size.h as f64 * zoom) as i32 + bw * 2;
    let color = if Some(cw.id) == fid { 
        convert_color(config.focused_border_color) 
    } else { convert_color(config.unfocused_border_color) };


    let area = Rectangle { loc: (sx, sy).into(), size: (ww, wh).into() };

    PixelShaderElement::new(
        prog.clone(),
        area,
        None,
        1.0,
        vec![
            Uniform::new("u_color", color),
            Uniform::new("elem_size", (ww as f32, wh as f32)),
            Uniform::new("radius", config.corner_rounding * zoom as f32),
            Uniform::new("thickness", bw as f32)
        ],
        Kind::Unspecified,
    )
}

pub fn indicator_element(view_mode: ViewMode, prog: &GlesPixelProgram) -> PixelShaderElement {
    let color: (f32, f32, f32, f32) = match view_mode {
        ViewMode::Tiling   => (0.25, 0.85, 0.45, 0.85), // green
        ViewMode::TreeView => (0.35, 0.60, 1.00, 0.85), // blue
        ViewMode::Fullscreen => (1.00, 0.25, 0.25, 0.85),
    };
    let area = Rectangle {
        loc: (12, 12).into(),
        size: (18, 18).into(),
    };
    PixelShaderElement::new(
        prog.clone(),
        area,
        None,
        1.0,
        vec![Uniform::new("u_color", color)],
        Kind::Unspecified,
    )
}

pub fn draw_cursor(
    cursor_position: Point<f64, Logical>,
    cursor_texture: GlesTexture, 
    renderer: &mut GlesRenderer,
    scale: f64,
    config: &AeroWMConfig, 
) -> TextureRenderElement<GlesTexture> {
    let buffer = TextureBuffer::from_texture(
        renderer,
        cursor_texture,
        1,
        Transform::Normal,
        None,
    );
    TextureRenderElement::from_texture_buffer(
        cursor_position.to_physical_precise_round(scale),
        &buffer,
        Some(1.0_f32),
        None,
        Some(Size::new(config.cursor_size[0], config.cursor_size[1])),
        Kind::Unspecified,
    )
}

pub fn build_render_elements(
    output_name: &Option<String>,
    windows: &[CanvasWindow],
    or_windows: &[Window],
    space: &Space<Window>,
    view_mode: ViewMode,
    tiling_visible_ids: &[u32],
    scale: f64,
    zoom: f64,
    viewport_x: f64,
    viewport_y: f64,
    config: &AeroWMConfig,
    show_areas: &bool,
    areas: &HashMap<u32, Rectangle<f64, Logical>>,
    cursor_position: Point<f64, Logical>,
    cursor_texture: &Option<GlesTexture>,
    background_texture: &Option<GlesTexture>,
    background_shader_prog: &Option<GlesPixelProgram>,
    background_image_size: &Option<(i32, i32)>,
    elapsed_secs: f32,
    renderer: &mut GlesRenderer,
    line_prog: &Option<GlesPixelProgram>, 
    solid_prog: &Option<GlesPixelProgram>,
    border_prog: &Option<GlesPixelProgram>
) ->Vec<AeroWMElement> {
    // Assemble overlay elements for this frame.
    let mut overlays: Vec<AeroWMElement> = Vec::new();
    let output = space.outputs().next().unwrap().clone();

    if !cursor_texture.is_none() {
        overlays.push(AeroWMElement::Texture(draw_cursor(cursor_position, cursor_texture.clone().expect("cursor image undefined"), renderer, scale, config)));
    }

    // Render layer surfaces (wlr-layer-shell: background/bottom/top/overlay).
    {
        let layer_map = smithay::desktop::layer_map_for_output(&output);
        for layer in layer_map.layers() {
            let loc = layer_map.layer_geometry(layer).unwrap_or_default().loc;
            overlays.extend(
                layer.render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
                    renderer,
                    loc.to_physical_precise_round(scale),
                    Scale::from(scale),
                    1.0,
                ).into_iter().map(AeroWMElement::Surface)
            );
        }
    }

    let mut sorted_windows: Vec<&CanvasWindow> = windows.iter().collect();
    sorted_windows.sort_by_key(|cw| std::cmp::Reverse(cw.z_index));

    for window in sorted_windows {
        if &window.output_name != output_name { continue; }
        if view_mode == ViewMode::Tiling && !tiling_visible_ids.contains(&window.id) && !(window.is_scratchpad && window.scratchpad_visible) {continue;}
        if view_mode == ViewMode::Fullscreen && !window.is_fullscreen { continue; }
        if window.is_scratchpad && !window.scratchpad_visible { continue; };
        
        let sx = ((window.canvas_x - viewport_x) * zoom) as i32;
        let sy = ((window.canvas_y - viewport_y) * zoom) as i32;
        let screen_loc = Point::from((sx, sy));
        let geom_offset = window.window.geometry().loc;
        let surface_phys = (screen_loc - geom_offset).to_physical_precise_round(scale);
        let phys_loc = screen_loc.to_physical_precise_round(scale);

        let geo = Rectangle { 
            loc: screen_loc, 
            size: window.window.geometry().size
        };
        
        if let Some(prog) = &border_prog {
            overlays.push(AeroWMElement::Shader(focus_border_elements(Some(window.id), config.clone(), zoom, prog, window, geo)));
        }
        overlays.extend(
            window.window.render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
                renderer,
                surface_phys,
                Scale::from(scale),
                1.0,
            ).into_iter().map(|e| AeroWMElement::ScaledSurface(
                RescaleRenderElement::from_element(e, phys_loc, Scale::from(zoom))
            ))
        );
    }

    for w in or_windows {
        if let Some(geo) = space.element_geometry(w) {
            let geom_offset = w.geometry().loc;
            let surface_phys = (geo.loc - geom_offset).to_physical_precise_round(scale);
            overlays.extend(
                w.render_elements(
                    renderer, 
                    surface_phys, 
                    Scale::from(scale), 
                    1.0,
                ).into_iter().map(AeroWMElement::Surface)
            );
        }
    }

    let mut push_strip = |loc: Point<f64, Logical>, size: Size<i32, Logical>, color: [u8; 4]| {
        let color = convert_color(color);
        let buf = SolidColorBuffer::new(size, Color32F::new(color[0], color[1], color[2], color[3]));
        let elem = SolidColorRenderElement::from_buffer(
            &buf, 
            loc.to_physical_precise_round(scale), 
            scale, 
            1.0, 
            Kind::Unspecified,
        );
        overlays.push(AeroWMElement::Solid(elem));
    };
    if *show_areas || config.always_show_areas {
        for (id, rect) in areas {
            let color = config.area_colors[(*id as usize - 1) % config.area_colors.len()];
            let sx = (rect.loc.x - viewport_x) * zoom;
            let sy = (rect.loc.y - viewport_y) * zoom;
            let sw = rect.size.w * zoom;
            let sh = rect.size.h * zoom;
            let t = config.area_border_thickness;
            push_strip(Point::new(sx, sy), Size::new(sw as i32, t), color);
            push_strip(Point::new(sx, sy + sh - t as f64), Size::new(sw as i32, t), color);
            push_strip(Point::new(sx, sy), Size::new(t, sh  as i32), color);
            push_strip(Point::new(sx + sw - t as f64, sy), Size::new(t, sh as i32), color);
        }
    }

    if let Some(prog) = &solid_prog {
        overlays.push(AeroWMElement::Shader(indicator_element(view_mode, prog)));
    }
    if view_mode == ViewMode::TreeView {
        if let Some(prog) = &line_prog {
            overlays.extend(connector_elements(windows, zoom, viewport_x, viewport_y, prog).into_iter().map(AeroWMElement::Shader));
        }
    }

    match config.background_type.as_str() {
        "image" => {
            if let Some(tex) = background_texture {
                let (sw, sh) = {
                    let o = space.outputs().next().unwrap();
                    let g = space.output_geometry(o).unwrap(); 
                    (g.size.w, g.size.h)
                };

                let buffer = TextureBuffer::from_texture(
                    renderer, 
                    tex.clone(), 
                    1, 
                    Transform::Normal, 
                    None,
                );
                let (iw, ih) = background_image_size.expect("Background image size not passed to build_render_elements");
                let bg_scale = f64::min(sw as f64 / iw as f64, sh as f64 / ih as f64);
                let tile_w = (iw as f64 * bg_scale) as i32;
                let tile_h = (ih as f64 * bg_scale) as i32;

                let i_start = (viewport_x / tile_w as f64).floor() as i32;
                let i_end = ((viewport_x + sw as f64 / zoom) / tile_w as f64).ceil() as i32;
                let j_start = (viewport_y / tile_h as f64).floor() as i32;
                let j_end = ((viewport_y + sh as f64 / zoom) / tile_h as f64).ceil() as i32;

                for j in j_start..=j_end {
                    for i in i_start..=i_end {
                        let screen_x = (i as f64 * tile_w as f64 - viewport_x) * zoom;
                        let screen_y = (j as f64 * tile_h as f64 - viewport_y) * zoom;
                        overlays.push(AeroWMElement::Texture(
                            TextureRenderElement::from_texture_buffer(
                                Point::from((screen_x, screen_y)).to_physical_precise_round(scale),
                                &buffer,
                                Some(1.0_f32),
                                Some(Rectangle { loc: (0.0, 0.0).into(), size: (iw as f64, ih as f64).into() }),
                                Some(Size::new((tile_w as f64 * zoom) as i32, (tile_h as f64 * zoom) as i32)),
                                Kind::Unspecified,
                            )
                        ));
                    }
                }

            }
        }
        "shader" => {
            if let Some(prog) = background_shader_prog {
                let (sw, sh) = {
                    let o = space.outputs().next().unwrap();
                    let g = space.output_geometry(o).unwrap();
                    (g.size.w, g.size.h)
                };
                overlays.push(AeroWMElement::Shader(PixelShaderElement::new(
                    prog.clone(),
                    Rectangle { loc: (0, 0).into(), size: (sw, sh).into() },
                    None,
                    1.0,
                    vec![
                        Uniform::new("u_time", elapsed_secs),
                        Uniform::new("u_resolution", (sw as f32, sh as f32)),
                        Uniform::new("u_viewport", (viewport_x as f32, viewport_y as f32)),
                        Uniform::new("u_zoom", zoom as f32),
                    ],
                    Kind::Unspecified,
                )));
            }
        }
        _ => {}
    }

    overlays
}