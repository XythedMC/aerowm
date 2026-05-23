precision highp float;
varying vec2 v_coords;
uniform float u_time;
uniform vec2  u_resolution;
uniform vec2  u_viewport;
uniform float u_zoom;

float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }
float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1, 0)), f.x),
               mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), f.x), f.y);
}

void main() {
    vec2 uv = v_coords;

    vec3 top    = vec3(0.18, 0.10, 0.30);
    vec3 mid    = vec3(0.85, 0.45, 0.40);
    vec3 bottom = vec3(0.95, 0.75, 0.45);

    vec3 col = mix(bottom, mid, smoothstep(0.0, 0.6, uv.y));
    col      = mix(col, top, smoothstep(0.6, 1.0, uv.y));

    // Slow drifting clouds.
    vec2 canvas = uv * u_resolution / u_zoom + u_viewport;
    float n = noise(canvas * 0.003 + vec2(u_time * 0.02, 0.0));
    n      += 0.5 * noise(canvas * 0.006 + vec2(u_time * 0.04, 0.0));
    float cloud = smoothstep(0.55, 0.85, n) * smoothstep(0.2, 0.7, uv.y);
    col   = mix(col, vec3(0.95, 0.85, 0.80), cloud * 0.5);

    gl_FragColor = vec4(col, 1.0);
}
