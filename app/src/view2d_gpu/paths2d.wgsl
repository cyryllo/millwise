// Ścieżki narzędzia w widoku 2D: każdy odcinek to jeden prostokąt (instancja),
// a fragment liczy odległość od odcinka (kapsuła) — gładkie krawędzie i
// zaokrąglone połączenia bez MSAA. Mieszanie „max” sprawia, że nakładające się
// odcinki (połączenia, gęste przejścia) nie ciemnieją ani nie jaśnieją.

struct U {
    // środek widoku [mm], piksele na mm (fizyczne), rozmiar tekstury [px]
    center: vec2<f32>,
    scale: f32,
    ppp: f32,
    size: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(0) var<uniform> u: U;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    // współrzędne względem początku odcinka [px]: wzdłuż, w poprzek
    @location(0) local: vec2<f32>,
    // długość odcinka [px], połowa szerokości [px], krycie cienkiej linii, przerywana (0/1)
    @location(1) shape: vec4<f32>,
    @location(2) color: vec4<f32>,
};

fn to_px(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        u.size.x * 0.5 + (p.x - u.center.x) * u.scale,
        u.size.y * 0.5 - (p.y - u.center.y) * u.scale,
    );
}

// seg: a.xy, b.xy [mm]; params.x: szerokość, params.y: rodzaj
// (0 — szerokość w punktach ekranu, 1 — w mm (pas freza), 2 — przerywana, w punktach)
@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) seg: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) params: vec2<f32>,
) -> VOut {
    var out: VOut;
    let pa = to_px(seg.xy);
    let pb = to_px(seg.zw);
    let d = pb - pa;
    let len = length(d);
    let mode = params.y;
    var w = params.x * u.ppp;
    if (mode > 0.5 && mode < 1.5) {
        w = params.x * u.scale;
    }
    // pas węższy niż 2 punkty i krótkie ruchy szybkie (jak w rysowaniu przez procesor) — pomijane
    let skip = (mode > 0.5 && mode < 1.5 && w < 2.0 * u.ppp) || (mode > 1.5 && len <= u.ppp);
    if (skip) {
        out.pos = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        out.local = vec2<f32>(0.0);
        out.shape = vec4<f32>(0.0);
        out.color = vec4<f32>(0.0);
        return out;
    }
    var dir = vec2<f32>(1.0, 0.0);
    if (len > 1e-4) {
        dir = d / len;
    }
    let n = vec2<f32>(-dir.y, dir.x);
    // linia cieńsza niż piksel: szerokość 1 px i proporcjonalnie mniejsze krycie
    let half = max(w, 1.0) * 0.5;
    let cover = min(w, 1.0);
    let m = half + 1.0;
    // 6 wierzchołków: dwa trójkąty prostokąta wydłużonego o m na końcach
    var corner = array<vec2<f32>, 6>(
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corner[vi];
    var along = -m;
    if (c.x > 0.5) {
        along = len + m;
    }
    let across = c.y * m;
    let p = pa + dir * along + n * across;
    out.pos = vec4<f32>(p.x / u.size.x * 2.0 - 1.0, 1.0 - p.y / u.size.y * 2.0, 0.0, 1.0);
    out.local = vec2<f32>(along, across);
    var dashed = 0.0;
    if (mode > 1.5) {
        dashed = 1.0;
    }
    out.shape = vec4<f32>(len, half, cover, dashed);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let len = in.shape.x;
    let half = in.shape.y;
    let t = in.local.x;
    var dist = abs(in.local.y);
    if (t < 0.0) {
        dist = length(in.local);
    } else if (t > len) {
        dist = length(vec2<f32>(t - len, in.local.y));
    }
    var a = clamp(half + 0.5 - dist, 0.0, 1.0) * in.shape.z;
    // przerywana: kreska 4 i przerwa 4 punkty, licząc od początku odcinka
    if (in.shape.w > 0.5) {
        let period = 8.0 * u.ppp;
        let k = max(t, 0.0) / period;
        if (t > len || fract(k) > 0.5) {
            discard;
        }
    }
    if (a <= 0.0) {
        discard;
    }
    return in.color * a;
}
