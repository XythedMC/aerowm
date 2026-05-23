precision highp float;
varying vec2 v_coords;
uniform float u_time;
uniform vec2  u_resolution;
uniform vec2  u_viewport;
uniform float u_zoom;

void main() {
    vec2 canvas_px = v_coords * u_resolution / u_zoom + u_viewport;

    float cell = 80.0;
    vec2 g = mod(canvas_px, cell);
    float line_w = 1.5 / u_zoom;

    float v = step(g.x, line_w) + step(g.y, line_w);
    v = clamp(v, 0.0, 1.0);

    vec2 maj = mod(canvas_px, cell * 5.0);
    float maj_w = 2.5 / u_zoom;
    float vm = step(maj.x, maj_w) + step(maj.y, maj_w);
    vm = clamp(vm, 0.0, 1.0);

    vec3 bg   = vec3(0.06, 0.07, 0.09);
    vec3 line = vec3(0.18, 0.22, 0.28);
    vec3 majc = vec3(0.35, 0.55, 0.85);
    vec3 col  = mix(bg, line, v);
    col       = mix(col, majc, vm * 0.8);

    gl_FragColor = vec4(col, 1.0);
}
