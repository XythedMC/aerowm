precision highp float;
varying vec2 v_coords;
uniform float u_time;
uniform vec2  u_resolution;
uniform vec2  u_viewport;
uniform float u_zoom;

void main() {
    vec2 canvas = v_coords * u_resolution / u_zoom + u_viewport;
    vec2 p = canvas / 250.0;

    float t = u_time * 0.4;
    float v = 0.0;
    for (int i = 0; i < 6; i++) {
        float fi = float(i);
        vec2  c  = vec2(sin(t * (0.5 + fi * 0.13) + fi),
                        cos(t * (0.4 + fi * 0.17) - fi));
        float d  = length(p - c * 1.5);
        v       += 1.0 / (0.5 + d * d * 2.0);
    }

    float k    = smoothstep(0.6, 2.5, v);
    vec3  bg   = vec3(0.04, 0.02, 0.10);
    vec3  blob = mix(vec3(0.95, 0.30, 0.15), vec3(1.00, 0.85, 0.30), k);
    vec3  col  = mix(bg, blob, smoothstep(0.6, 1.8, v));
    gl_FragColor = vec4(col, 1.0);
}
