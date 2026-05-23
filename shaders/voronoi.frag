precision highp float;
varying vec2 v_coords;
uniform float u_time;
uniform vec2  u_resolution;
uniform vec2  u_viewport;
uniform float u_zoom;

vec2 hash2(vec2 p) {
    p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
    return fract(sin(p) * 43758.5453);
}

void main() {
    vec2 p = (v_coords * u_resolution / u_zoom + u_viewport) / 120.0;

    vec2 ip = floor(p);
    vec2 fp = fract(p);

    float d1 = 8.0;
    float d2 = 8.0;
    for (int j = -1; j <= 1; j++) {
        for (int i = -1; i <= 1; i++) {
            vec2 g = vec2(float(i), float(j));
            vec2 o = hash2(ip + g);
            o = 0.5 + 0.5 * sin(u_time * 0.6 + 6.28 * o);
            vec2 r = g + o - fp;
            float d = dot(r, r);
            if (d < d1) { d2 = d1; d1 = d; } else if (d < d2) { d2 = d; }
        }
    }

    float edge = sqrt(d2) - sqrt(d1);
    vec3 col   = mix(vec3(0.08, 0.10, 0.18), vec3(0.30, 0.55, 0.90), sqrt(d1));
    col       += smoothstep(0.08, 0.0, edge) * vec3(0.9, 0.7, 0.5) * 0.3;
    gl_FragColor = vec4(col, 1.0);
}
