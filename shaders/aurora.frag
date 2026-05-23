precision highp float;
varying vec2 v_coords;
uniform float u_time;
uniform vec2  u_resolution;
uniform vec2  u_viewport;
uniform float u_zoom;

float hash(float n) { return fract(sin(n) * 43758.5453); }
float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = hash(dot(i, vec2(1.0, 57.0)));
    float b = hash(dot(i + vec2(1, 0), vec2(1.0, 57.0)));
    float c = hash(dot(i + vec2(0, 1), vec2(1.0, 57.0)));
    float d = hash(dot(i + vec2(1, 1), vec2(1.0, 57.0)));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

void main() {
    vec2 uv = v_coords;
    vec2 canvas = (uv * u_resolution / u_zoom + u_viewport) * 0.002;

    float n   = noise(vec2(canvas.x * 2.0, canvas.y * 3.0 + u_time * 0.2));
    float band = smoothstep(0.0, 0.3, n - uv.y + 0.4);
    band      *= smoothstep(0.0, 0.3, uv.y - n + 0.1);

    vec3 sky = mix(vec3(0.02, 0.02, 0.08), vec3(0.04, 0.02, 0.12), uv.y);
    vec3 aur = mix(vec3(0.1, 0.9, 0.5), vec3(0.4, 0.3, 0.9), uv.y);
    gl_FragColor = vec4(sky + aur * band, 1.0);
}
