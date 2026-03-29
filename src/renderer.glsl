// calcNormal, softShadow, main — uniforms are declared in shader.rs above map()

vec3 calcNormal(vec3 p) {
    float e = 0.0001;
    return normalize(vec3(
        map(p + vec3(e, 0.0, 0.0)) - map(p - vec3(e, 0.0, 0.0)),
        map(p + vec3(0.0, e, 0.0)) - map(p - vec3(0.0, e, 0.0)),
        map(p + vec3(0.0, 0.0, e)) - map(p - vec3(0.0, 0.0, e))
    ));
}

float softShadow(vec3 ro, vec3 rd, float mint, float maxt, float k) {
    float res = 1.0;
    float t = mint;
    for (int i = 0; i < 32; i++) {
        float h = map(ro + rd * t);
        if (h < 0.001) return 0.0;
        res = min(res, k * h / t);
        t += clamp(h, 0.02, 0.3);
        if (t > maxt) break;
    }
    return clamp(res, 0.0, 1.0);
}

void main() {
    vec2 uv = (gl_FragCoord.xy - 0.5 * iResolution) / min(iResolution.x, iResolution.y);

    // Camera: looking along -Z, positioned at +Z.
    // Focal length 1.5 gives ~44-degree FOV regardless of object size.
    vec3 ro = vec3(0.0, 0.0, iCameraZ);
    vec3 rd = normalize(vec3(uv, -1.5));

    // Ray march
    float t = 0.0;
    float tmax = iCameraZ * 4.0;
    bool hit = false;
    for (int i = 0; i < 128; i++) {
        float h = map(ro + rd * t);
        if (h < 0.0002 * t) { hit = true; break; }
        if (t > tmax) break;
        t += h;
    }

    vec3 col = vec3(0.12); // background
    if (hit) {
        vec3 pos = ro + rd * t;
        vec3 n   = calcNormal(pos);
        vec3 ld  = normalize(vec3(1.0, 2.0, 1.5));

        float diff = clamp(dot(n, ld), 0.0, 1.0);
        float amb  = 0.5 + 0.5 * n.y;
        float sha  = softShadow(pos + n * 0.002, ld, 0.01, iCameraZ * 2.0, 16.0);

        col = vec3(0.7) * (diff * sha + 0.3 * amb);
        col = pow(col, vec3(0.4545)); // gamma
    }

    gl_FragColor = vec4(col, 1.0);
}
