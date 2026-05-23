precision highp float;
varying vec2 v_coords;
uniform float u_time;
uniform vec2  u_resolution;
uniform vec2  u_viewport;
uniform float u_zoom;

void main() {
    vec2 canvas = v_coords * u_resolution / u_zoom + u_viewport;

    vec2 centres[3];
    centres[0] = vec2(   0.0,   0.0);
    centres[1] = vec2( 500.0, 300.0);
    centres[2] = vec2(-400.0, 600.0);

    float v = 0.0;
    for (int i = 0; i < 3; i++) {
        float d = length(canvas - centres[i]);
        v += sin(d * 0.04 - u_time * 1.5 - float(i)) / (1.0 + d * 0.002);
    }
    v *= 0.5;

    vec3 col = mix(vec3(0.03, 0.06, 0.10), vec3(0.25, 0.50, 0.85), 0.5 + 0.5 * v);
    col     += smoothstep(0.7, 1.0, v) * vec3(0.9, 0.9, 1.0) * 0.4;
    gl_FragColor = vec4(col, 1.0);
}
