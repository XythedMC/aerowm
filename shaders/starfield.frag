precision highp float;
varying vec2 v_coords;
uniform float u_time;
uniform vec2  u_resolution;
uniform vec2  u_viewport;
uniform float u_zoom;

float hash(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

void main() {
    vec2 canvas_px = v_coords * u_resolution / u_zoom + u_viewport;
    vec2 cell_size = vec2(30.0);
    vec2 cell      = floor(canvas_px / cell_size);
    vec2 local     = fract(canvas_px / cell_size);

    float r       = hash(cell);
    vec2  centre  = vec2(r, hash(cell + 17.0));
    float d       = length(local - centre);
    float twinkle = 0.5 + 0.5 * sin(u_time * (1.0 + r * 3.0) + r * 6.28);
    float bright  = smoothstep(0.08, 0.0, d) * twinkle;

    vec3 sky = mix(vec3(0.02, 0.03, 0.08), vec3(0.05, 0.05, 0.15), v_coords.y);
    vec3 col = sky + vec3(bright);
    gl_FragColor = vec4(col, 1.0);
}
