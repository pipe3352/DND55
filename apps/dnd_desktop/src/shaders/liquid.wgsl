// DND55 animated liquids: water (shallow/deep) and lava, drawn as one quad over the whole map.
// A per-cell data texture (linear filtering) gives smooth, organic shorelines; everything else is
// procedural noise, so there are no image assets. Quality (params.a.y) scales the noise octaves.

struct LiquidParams {
    a: vec4<f32>, // time, quality (0..3), map width (cells), map height (cells)
    b: vec4<f32>, // reduced motion, unused...
};

@group(2) @binding(0) var<uniform> params: LiquidParams;
@group(2) @binding(1) var cells: texture_2d<f32>;
@group(2) @binding(2) var cells_sampler: sampler;

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

fn hash2(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 5; i = i + 1) {
        if (i >= octaves) { break; }
        sum = sum + amp * vnoise(q);
        q = q * 2.03 + vec2<f32>(1.7, 9.2);
        amp = amp * 0.5;
    }
    return sum;
}

// Cellular noise: x = distance to nearest feature, y = distance to the second nearest.
fn cellular(p: vec2<f32>, t: f32) -> vec2<f32> {
    let i = floor(p);
    let f = fract(p);
    var d1 = 8.0;
    var d2 = 8.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let g = vec2<f32>(f32(x), f32(y));
            let h = hash2(i + g);
            let o = 0.5 + 0.4 * sin(vec2<f32>(t * 0.35 + 6.2831 * h, t * 0.27 + 6.2831 * fract(h * 7.3)));
            let d = length(g + o - f);
            if (d < d1) { d2 = d1; d1 = d; } else if (d < d2) { d2 = d; }
        }
    }
    return vec2<f32>(d1, d2);
}

@fragment
fn fragment(in: VOut) -> @location(0) vec4<f32> {
    let t = params.a.x;
    let quality = params.a.y;
    let size = params.a.zw;
    let octaves = 2 + i32(quality);
    let p = in.uv * size; // position in cells

    // Organic shoreline: perturb the lookup with low-frequency noise before sampling.
    let wob = vec2<f32>(fbm(p * 0.9 + vec2<f32>(3.1, 0.0), 3), fbm(p * 0.9 + vec2<f32>(0.0, 7.7), 3)) - 0.5;
    let s = textureSample(cells, cells_sampler, in.uv + wob * 0.75 / size);
    let water = s.r + s.g;
    let deep = s.g;
    let lava = s.b;

    if (max(water, lava) < 0.5) {
        discard;
    }

    if (lava >= water) {
        // ---------------- lava: drifting cooled crust plates over glowing magma
        let flow = p * 1.15 + vec2<f32>(t * 0.05, t * 0.03) + (fbm(p * 0.7 + t * 0.04, 2) - 0.5) * 1.2;
        let cell = cellular(flow, t);
        let crack = cell.y - cell.x;                      // small near plate borders
        let heat = fbm(p * 2.0 - vec2<f32>(t * 0.12, t * 0.08), octaves);
        let pulse = 0.85 + 0.15 * sin(t * 1.7 + heat * 6.0);
        let magma = mix(vec3<f32>(0.85, 0.18, 0.04), vec3<f32>(1.0, 0.78, 0.30), clamp(heat * 1.3 - 0.15, 0.0, 1.0)) * pulse;
        let crust = mix(vec3<f32>(0.10, 0.05, 0.04), vec3<f32>(0.24, 0.10, 0.06), heat);
        // Thicker crust towards the pool's edge.
        let edge = smoothstep(0.5, 0.9, lava);
        let open = smoothstep(0.02, 0.14 + 0.1 * (1.0 - edge), crack);
        var col = mix(magma, crust, open * (0.55 + 0.45 * (1.0 - edge)) + (1.0 - edge) * 0.25);
        // Glowing seams and bright pinpricks.
        col = col + vec3<f32>(1.0, 0.45, 0.1) * (1.0 - smoothstep(0.0, 0.06, crack)) * 0.6 * pulse;
        let spark = step(0.985, vnoise(p * 9.0 + vec2<f32>(0.0, t * 1.5)));
        col = col + vec3<f32>(1.0, 0.9, 0.5) * spark * 0.5;
        let a = smoothstep(0.5, 0.56, lava);
        return vec4<f32>(col, a);
    }

    // ---------------- water
    let d = clamp(deep * 1.15 + (water - 0.5) * 0.25, 0.0, 1.0);
    let shallow_c = vec3<f32>(0.16, 0.42, 0.44);
    let deep_c = vec3<f32>(0.07, 0.20, 0.30);
    var col = mix(shallow_c, deep_c, d);
    // Two layers of waves moving in different directions.
    let w1 = fbm(p * 0.9 + vec2<f32>(t * 0.16, t * 0.07), octaves);
    let w2 = fbm(p * 1.8 - vec2<f32>(t * 0.11, -t * 0.13), octaves);
    let w = w1 * 0.6 + w2 * 0.4;
    col = col * (0.85 + 0.3 * w);
    // Caustic-like light ridges (stronger in the shallows) and sky glints.
    let ridge = pow(1.0 - abs(sin(w * 11.0 + t * 0.6)), 10.0);
    col = col + vec3<f32>(0.55, 0.75, 0.75) * ridge * (0.35 - 0.25 * d);
    let glint = smoothstep(0.78, 0.86, fbm(p * 3.5 + vec2<f32>(t * 0.4, t * 0.2), 2)) * (0.5 + 0.5 * sin(t * 3.0 + p.x));
    col = col + vec3<f32>(0.9, 0.95, 1.0) * glint * 0.25;
    // Shoreline: ripples travelling outward plus broken foam at the very edge.
    let shore = 1.0 - smoothstep(0.5, 0.66, water);
    let band = 0.5 + 0.5 * sin(water * 38.0 - t * 2.2);
    let foam_noise = vnoise(p * 6.0 + vec2<f32>(t * 0.6, 0.0));
    let foam = shore * smoothstep(0.35, 0.95, band * (0.5 + foam_noise));
    col = mix(col, vec3<f32>(0.86, 0.92, 0.90), clamp(foam * 0.7 + smoothstep(0.56, 0.5, water) * 0.3, 0.0, 1.0));
    // Slightly see-through in the shallows so the bed shows through at the edges.
    let a = smoothstep(0.5, 0.55, water) * mix(0.9, 0.98, d);
    return vec4<f32>(col, a);
}
