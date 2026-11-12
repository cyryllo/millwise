struct Uniforms {
    view_proj: mat4x4<f32>,
    // xyz = kierunek światła (do źródła), w = światło otoczenia
    light: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

// ---------- linie ----------
struct LineIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_line(i: LineIn) -> LineOut {
    var o: LineOut;
    o.clip = u.view_proj * vec4<f32>(i.pos, 1.0);
    o.color = i.color;
    return o;
}

@fragment
fn fs_line(i: LineOut) -> @location(0) vec4<f32> {
    return i.color;
}

// ---------- siatki trójkątów ----------
struct MeshIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_mesh(i: MeshIn) -> MeshOut {
    var o: MeshOut;
    o.clip = u.view_proj * vec4<f32>(i.pos, 1.0);
    o.normal = i.normal;
    o.color = i.color;
    return o;
}

@fragment
fn fs_mesh(i: MeshOut) -> @location(0) vec4<f32> {
    let n = normalize(i.normal);
    let l = normalize(u.light.xyz);
    // oświetlenie dwustronne, żeby wnętrza też były czytelne
    let diffuse = abs(dot(n, l));
    let k = u.light.w + (1.0 - u.light.w) * diffuse;
    // kolory są premultiplied — mnożymy tylko rgb
    return vec4<f32>(i.color.rgb * k, i.color.a);
}
