// Symulacja ubytku materiału na karcie graficznej.
//
// 1. Przejazdy narzędzia rysowane z góry (rzut prostokątny) do tekstury wysokości
//    (R32Float) z buforem głębokości: test „mniejsze” zostawia w każdym pikselu
//    najniższy punkt narzędzia — czyli powierzchnię materiału po obróbce. (Tekstury
//    głębokości nie da się czytać w shaderze przez OpenGL — stąd osobna tekstura liczb.)
// 2. Powierzchnia materiału wyświetlana wprost z tej tekstury (siatka bez bufora:
//    położenie węzła z numeru wierzchołka, wysokość i normalna z tekstury).

// ---------- 1. przejazdy ----------
struct Sweep {
    // xy = lewy dolny róg obszaru, zw = jego rozmiar [mm]
    area: vec4<f32>,
    // x = Z dla głębokości 0, y = Z dla głębokości 1 (góra materiału)
    z: vec4<f32>,
};
@group(0) @binding(0) var<uniform> sw: Sweep;

struct SweepIn {
    // punkt dolnej powierzchni narzędzia względem czubka; flaga 0 = początek ruchu, 1 = koniec
    @location(0) tpos: vec3<f32>,
    @location(1) flag: f32,
    // ruch: początek i koniec (dane instancji)
    @location(2) a: vec3<f32>,
    @location(3) b: vec3<f32>,
};

struct SweepOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) z: f32,
};

@vertex
fn vs_sweep(i: SweepIn) -> SweepOut {
    let p = i.tpos + mix(i.a, i.b, i.flag);
    let u = (p.xy - sw.area.xy) / sw.area.zw;
    let z = (p.z - sw.z.x) / (sw.z.y - sw.z.x);
    var o: SweepOut;
    // wiersz 0 tekstury = najmniejsze Y
    o.clip = vec4<f32>(u.x * 2.0 - 1.0, 1.0 - u.y * 2.0, z, 1.0);
    o.z = p.z;
    return o;
}

// wysokość zapisywana jako liczba (tekstura R32Float); test głębokości zostawia najniższą
@fragment
fn fs_sweep(i: SweepOut) -> @location(0) vec4<f32> {
    return vec4<f32>(i.z, 0.0, 0.0, 1.0);
}

// ---------- 2. powierzchnia ----------
struct Disp {
    view_proj: mat4x4<f32>,
    // xyz = kierunek światła, w = światło otoczenia
    light: vec4<f32>,
    // xy = lewy dolny róg obszaru, zw = rozmiar [mm]
    area: vec4<f32>,
    // xy = rozmiar tekstury (piksele), zw = rozmiar siatki wyświetlania (węzły)
    dims: vec4<f32>,
    // x = Z dla głębokości 0, y = Z dla głębokości 1, z = spód materiału, w = góra materiału
    zs: vec4<f32>,
};
@group(0) @binding(0) var<uniform> d: Disp;
@group(0) @binding(1) var hmap: texture_2d<f32>;

struct SOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    // > 0.5 — przelotowo (materiał zdjęty do spodu): bez powierzchni
    @location(2) thru: f32,
};

fn height(t: vec2<i32>) -> f32 {
    let n = vec2<i32>(i32(d.dims.x), i32(d.dims.y));
    let c = clamp(t, vec2<i32>(0, 0), n - vec2<i32>(1, 1));
    return min(textureLoad(hmap, c, 0).r, d.zs.w);
}

// węzeł siatki wyświetlania -> piksel tekstury
fn texel_of(g: vec2<i32>) -> vec2<i32> {
    let tn = d.dims.xy - vec2<f32>(1.0, 1.0);
    let gn = max(d.dims.zw - vec2<f32>(1.0, 1.0), vec2<f32>(1.0, 1.0));
    return vec2<i32>(round(vec2<f32>(g) * tn / gn));
}

fn world_xy(t: vec2<i32>) -> vec2<f32> {
    let cell = d.area.zw / d.dims.xy;
    return d.area.xy + (vec2<f32>(t) + vec2<f32>(0.5, 0.5)) * cell;
}

// kolory (liniowe): góra drewna, dno wycięć jaśniejsze płytko / ciemniejsze głęboko, boki
fn surface_color(z: f32) -> vec3<f32> {
    let wood_top = vec3<f32>(0.578, 0.371, 0.162);
    let cut_hi = vec3<f32>(0.445, 0.246, 0.0976);
    let cut_lo = vec3<f32>(0.188, 0.0931, 0.0343);
    if (z >= d.zs.w - 0.001) {
        return wood_top;
    }
    let f = clamp((d.zs.w - z) / max(d.zs.w - d.zs.z, 0.001), 0.0, 1.0);
    return mix(cut_hi, cut_lo, f);
}

const SIDE: vec3<f32> = vec3<f32>(0.305, 0.162, 0.0612);

// górna powierzchnia (instancja 0) i spód (instancja 1): siatka komórek po 2 trójkąty
@vertex
fn vs_surface(@builtin(vertex_index) vi: u32, @builtin(instance_index) mode: u32) -> SOut {
    var corners = array<vec2<i32>, 6>(
        vec2<i32>(0, 0), vec2<i32>(1, 0), vec2<i32>(1, 1),
        vec2<i32>(0, 0), vec2<i32>(1, 1), vec2<i32>(0, 1),
    );
    let gnx = u32(d.dims.z);
    let cell = vi / 6u;
    let g = vec2<i32>(i32(cell % (gnx - 1u)), i32(cell / (gnx - 1u))) + corners[vi % 6u];
    let t = texel_of(g);
    let xy = world_xy(t);
    let z = height(t);
    var o: SOut;
    o.thru = select(0.0, 1.0, z <= d.zs.z + 0.005);
    if (mode == 0u) {
        // normalna z sąsiednich pikseli (krok = piksele na węzeł)
        let s = max(i32(round(d.dims.x / max(d.dims.z, 1.0))), 1);
        let cellw = d.area.zw / d.dims.xy;
        let hx = (height(t + vec2<i32>(s, 0)) - height(t - vec2<i32>(s, 0))) / (2.0 * f32(s) * cellw.x);
        let hy = (height(t + vec2<i32>(0, s)) - height(t - vec2<i32>(0, s))) / (2.0 * f32(s) * cellw.y);
        o.normal = normalize(vec3<f32>(-hx, -hy, 1.0));
        o.color = surface_color(z);
        o.clip = d.view_proj * vec4<f32>(xy, max(z, d.zs.z), 1.0);
    } else {
        o.normal = vec3<f32>(0.0, 0.0, -1.0);
        o.color = SIDE;
        o.clip = d.view_proj * vec4<f32>(xy, d.zs.z, 1.0);
    }
    return o;
}

// boki półfabrykatu: pasy wzdłuż czterech krawędzi siatki, od powierzchni do spodu
@vertex
fn vs_skirt(@builtin(vertex_index) vi: u32) -> SOut {
    let gnx = i32(d.dims.z);
    let gny = i32(d.dims.w);
    var q = i32(vi / 6u);
    let k = vi % 6u;
    var g0: vec2<i32>;
    var g1: vec2<i32>;
    var n: vec3<f32>;
    if (q < gnx - 1) {
        g0 = vec2<i32>(q + 1, 0); g1 = vec2<i32>(q, 0); n = vec3<f32>(0.0, -1.0, 0.0);
    } else {
        q = q - (gnx - 1);
        if (q < gnx - 1) {
            g0 = vec2<i32>(q, gny - 1); g1 = vec2<i32>(q + 1, gny - 1); n = vec3<f32>(0.0, 1.0, 0.0);
        } else {
            q = q - (gnx - 1);
            if (q < gny - 1) {
                g0 = vec2<i32>(0, q); g1 = vec2<i32>(0, q + 1); n = vec3<f32>(-1.0, 0.0, 0.0);
            } else {
                q = q - (gny - 1);
                g0 = vec2<i32>(gnx - 1, q + 1); g1 = vec2<i32>(gnx - 1, q); n = vec3<f32>(1.0, 0.0, 0.0);
            }
        }
    }
    // narożniki pasa: (0, góra), (1, góra), (1, spód), (0, góra), (1, spód), (0, spód)
    var which = array<u32, 6>(0u, 1u, 1u, 0u, 1u, 0u);
    var up = array<bool, 6>(true, true, false, true, false, false);
    let g = select(g0, g1, which[k] == 1u);
    let t = texel_of(g);
    let xy = world_xy(t);
    let z = height(t);
    var o: SOut;
    o.thru = select(0.0, 1.0, z <= d.zs.z + 0.005);
    o.normal = n;
    o.color = SIDE;
    let zz = select(d.zs.z, max(z, d.zs.z), up[k]);
    o.clip = d.view_proj * vec4<f32>(xy, zz, 1.0);
    return o;
}

@fragment
fn fs_surface(i: SOut) -> @location(0) vec4<f32> {
    if (i.thru > 0.5) {
        discard;
    }
    let n = normalize(i.normal);
    let l = normalize(d.light.xyz);
    let diffuse = abs(dot(n, l));
    let k = d.light.w + (1.0 - d.light.w) * diffuse;
    return vec4<f32>(i.color * k, 1.0);
}
