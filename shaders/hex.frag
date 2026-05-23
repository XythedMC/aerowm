precision highp float;
varying vec2 v_coords;
uniform float u_time;
uniform vec2  u_resolution;
uniform vec2  u_viewport;
uniform float u_zoom;

float hex_dist(vec2 p) {
    p = abs(p);
    return max(dot(p, normalize(vec2(1.0, 1.732))), p.x);
}

void main() {
    vec2 p = (v_coords * u_resolution / u_zoom + u_viewport) / 60.0;

    vec2 r = vec2(1.0, 1.732);
    vec2 h = r * 0.5;
    vec2 a = mod(p, r) - h;
    vec2 b = mod(p - h, r) - h;
    vec2 gv = (dot(a, a) < dot(b, b)) ? a : b;

    float d = hex_dist(gv);
    float cell_id = (dot(a, a) < dot(b, b))
        ? dot(floor(p / r), vec2(13.7, 91.3))
        : dot(floor((p - h) / r), vec2(13.7, 91.3));
    float pulse = 0.5 + 0.5 * sin(u_time * 0.8 + cell_id);

    vec3 base   = vec3(0.05, 0.07, 0.12);
    vec3 glow   = mix(vec3(0.2, 0.5, 0.8), vec3(0.9, 0.4, 0.7), pulse);
    float edge  = smoothstep(0.50, 0.46, d);
    vec3 col    = mix(base, glow * 0.6, edge);
    col        += smoothstep(0.50, 0.49, d) * 0.0; // outline
    gl_FragColor = vec4(col, 1.0);
}
