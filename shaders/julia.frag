precision highp float;
varying vec2 v_coords;
uniform float u_time;
uniform vec2  u_resolution;
uniform vec2  u_viewport;
uniform float u_zoom;

void main() {
    vec2 canvas = v_coords * u_resolution / u_zoom + u_viewport;
    vec2 z = (canvas - u_resolution * 0.5) / 600.0;

    vec2 c = vec2(0.7885 * cos(u_time * 0.2), 0.7885 * sin(u_time * 0.2));

    float iter = 0.0;
    const float max_iter = 60.0;
    for (float i = 0.0; i < 60.0; i++) {
        z = vec2(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        if (dot(z, z) > 4.0) break;
        iter++;
    }

    float v   = iter / max_iter;
    vec3 col1 = vec3(0.05, 0.05, 0.15);
    vec3 col2 = vec3(0.30, 0.55, 0.90);
    vec3 col3 = vec3(0.95, 0.75, 0.35);
    vec3 col  = mix(col1, col2, v);
    col       = mix(col, col3, smoothstep(0.6, 1.0, v));
    if (iter >= max_iter) col = vec3(0.02);
    gl_FragColor = vec4(col, 1.0);
}
