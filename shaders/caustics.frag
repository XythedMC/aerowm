precision highp float;
varying vec2 v_coords;
uniform float u_time;
uniform vec2  u_resolution;
uniform vec2  u_viewport;
uniform float u_zoom;

void main() {
    vec2 p = (v_coords * u_resolution / u_zoom + u_viewport) / 200.0;
    float t = u_time * 0.5;

    vec2 q = p;
    float intensity = 0.0;
    for (int i = 0; i < 4; i++) {
        float fi = float(i);
        q.x += sin(p.y * (2.0 + fi) + t * (1.0 + fi * 0.3)) * 0.5;
        q.y += cos(p.x * (2.0 + fi) - t * (1.2 + fi * 0.2)) * 0.5;
        intensity += 1.0 / length(fract(q) - 0.5);
    }

    intensity = pow(intensity * 0.06, 1.5);
    vec3 col = mix(vec3(0.0, 0.10, 0.20), vec3(0.4, 0.85, 1.0), intensity);
    gl_FragColor = vec4(col, 1.0);
}
