//! Symulacja ubytku materiału na karcie graficznej (silnik „GPU”).
//!
//! Wysokość materiału to tekstura liczb (z buforem głębokości): każdy ruch narzędzia jest rysowany
//! z góry jako dokładny kształt przejazdu (dolna powierzchnia narzędzia na początku
//! i na końcu ruchu oraz powierzchnie zakreślane przez jej krawędzie), a test
//! głębokości zostawia w każdym pikselu najniższy punkt. Klatka dorysowuje tylko
//! nowe ruchy; cofnięcie czyści teksturę i rysuje od początku. Powierzchnię
//! materiału karta wyświetla wprost z tekstury — procesor nie buduje siatek.
//! Bez obliczeń ogólnych (compute) — zwykłe rysowanie, działa też przez OpenGL.

use bytemuck::{Pod, Zeroable};
use cam_ops::P3;
use cam_sim::{SimPoint, ToolShape};
use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;
use glam::Mat4;

use super::renderer::{COLOR_FORMAT, DEPTH_FORMAT, SAMPLES};

/// Wysokość materiału (liczba w każdym pikselu) i bufor głębokości do wyboru najniższej.
const HEIGHT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
const HDEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Odcinków w jednym buforze instancji (24 B każdy → 24 MB).
const INST_CHUNK: usize = 1 << 20;
/// Najwięcej węzłów siatki wyświetlania wzdłuż boku (tekstura może być gęstsza).
const DISPLAY_MAX: u32 = 1536;
/// Odcinków na boku dolnej powierzchni narzędzia.
const TOOL_SEGMENTS: usize = 40;

/// Dane wejściowe: oś czasu programu, narzędzia, półfabrykat, chwila.
pub struct GpuSimInput<'a> {
    pub timeline: &'a [SimPoint],
    pub tools: &'a [ToolShape],
    /// Półfabrykat z góry: x0, y0, x1, y1 [mm].
    pub area: [f64; 4],
    pub top: f64,
    pub bottom: f64,
    pub time: f64,
    /// Piksele tekstury wzdłuż dłuższego boku.
    pub resolution: u32,
    /// Zmiana = nowa symulacja (inne ścieżki).
    pub key: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TVert {
    pos: [f32; 3],
    flag: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Inst {
    a: [f32; 3],
    b: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SweepU {
    area: [f32; 4],
    z: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DispU {
    view_proj: [[f32; 4]; 4],
    light: [f32; 4],
    area: [f32; 4],
    dims: [f32; 4],
    zs: [f32; 4],
}

/// Siatka przejazdu narzędzia: dolna powierzchnia na początku (flaga 0) i końcu (1)
/// ruchu oraz czworokąty zakreślane przez jej krawędzie. Dla bryły wypukłej to dokładna
/// dolna obwiednia przejazdu po linii prostej.
fn tool_template(t: &ToolShape) -> Vec<TVert> {
    let s = TOOL_SEGMENTS;
    let rings: Vec<(f32, f32)> = t.ring_radii().into_iter().map(|r| (r as f32, t.profile(r).unwrap_or(0.0) as f32)).collect();
    let center = [0.0f32, 0.0, t.profile(0.0).unwrap_or(0.0) as f32];
    let ring_pt = |k: usize, i: usize| -> [f32; 3] {
        let (r, h) = rings[k];
        let a = (i % s) as f32 / s as f32 * std::f32::consts::TAU;
        [r * a.cos(), r * a.sin(), h]
    };
    // trójkąty dolnej powierzchni
    let mut faces: Vec<[[f32; 3]; 3]> = Vec::new();
    for i in 0..s {
        faces.push([center, ring_pt(0, i), ring_pt(0, i + 1)]);
        for k in 0..rings.len().saturating_sub(1) {
            let (p, q, r, w) = (ring_pt(k, i), ring_pt(k, i + 1), ring_pt(k + 1, i + 1), ring_pt(k + 1, i));
            faces.push([p, q, r]);
            faces.push([p, r, w]);
        }
    }
    // krawędzie: obwody pierścieni, a przy zakrzywionym profilu także promieniste
    let mut edges: Vec<([f32; 3], [f32; 3])> = Vec::new();
    for i in 0..s {
        for k in 0..rings.len() {
            edges.push((ring_pt(k, i), ring_pt(k, i + 1)));
        }
        if !t.is_flat() {
            edges.push((center, ring_pt(0, i)));
            for k in 0..rings.len().saturating_sub(1) {
                edges.push((ring_pt(k, i), ring_pt(k + 1, i)));
            }
        }
    }
    let v = |p: [f32; 3], flag: f32| TVert { pos: p, flag };
    let mut out = Vec::with_capacity(faces.len() * 6 + edges.len() * 6);
    for flag in [0.0, 1.0] {
        for f in &faces {
            out.extend(f.iter().map(|&p| v(p, flag)));
        }
    }
    for (p, q) in edges {
        out.extend([v(p, 0.0), v(q, 0.0), v(q, 1.0), v(p, 0.0), v(q, 1.0), v(p, 1.0)]);
    }
    out
}

/// Zasoby zależne od symulacji (ścieżek i rozdzielczości).
struct Built {
    key: (u64, u32),
    /// Rozmiar tekstury [px] i siatki wyświetlania [węzły].
    tex: [u32; 2],
    grid: [u32; 2],
    view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    disp_bg: wgpu::BindGroup,
    /// Wzorce narzędzi (bufor, liczba wierzchołków) dla operacji.
    templates: Vec<(wgpu::Buffer, u32)>,
    /// Instancje (ruchy robocze) w kawałkach po INST_CHUNK.
    inst: Vec<wgpu::Buffer>,
    /// Ciągłe zakresy instancji jednej operacji: (operacja, od, do).
    op_ranges: Vec<(usize, u32, u32)>,
    /// Liczba instancji dla odcinków osi czasu 1..=i.
    cum: Vec<u32>,
    sweep_u: SweepU,
    area: [f64; 4],
    top: f64,
    bottom: f64,
}

pub struct GpuSim {
    sweep_pipeline: wgpu::RenderPipeline,
    surface_pipeline: wgpu::RenderPipeline,
    skirt_pipeline: wgpu::RenderPipeline,
    sweep_bgl: wgpu::BindGroupLayout,
    disp_bgl: wgpu::BindGroupLayout,
    sweep_ubuf: wgpu::Buffer,
    sweep_bg: wgpu::BindGroup,
    disp_ubuf: wgpu::Buffer,
    partial_buf: wgpu::Buffer,
    built: Option<Built>,
    /// Ile instancji jest już w teksturze; czy wyczyścić przed rysowaniem.
    drawn: u32,
    clear: bool,
    last_time: f64,
    /// Do narysowania w tej klatce: zakres instancji i częściowy bieżący ruch (operacja).
    pending: (u32, u32),
    partial: Option<usize>,
}

impl GpuSim {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("millwise_gpu_sim"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_sim.wgsl").into()),
        });
        let ubuf_entry = |binding, vis| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let sweep_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("millwise_gpu_sweep_bgl"),
            entries: &[ubuf_entry(0, wgpu::ShaderStages::VERTEX)],
        });
        let disp_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("millwise_gpu_disp_bgl"),
            entries: &[
                ubuf_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let sweep_ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("millwise_gpu_sweep_u"),
            size: std::mem::size_of::<SweepU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sweep_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("millwise_gpu_sweep_bg"),
            layout: &sweep_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: sweep_ubuf.as_entire_binding() }],
        });
        let disp_ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("millwise_gpu_disp_u"),
            size: std::mem::size_of::<DispU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let partial_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("millwise_gpu_partial"),
            size: std::mem::size_of::<Inst>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // przejazdy: tylko głębokość (najniższy punkt), bez koloru
        let sweep_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("millwise_gpu_sweep_layout"),
            bind_group_layouts: &[Some(&sweep_bgl)],
            immediate_size: 0,
        });
        let tattrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32];
        let iattrs = wgpu::vertex_attr_array![2 => Float32x3, 3 => Float32x3];
        let sweep_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("millwise_gpu_sweep"),
            layout: Some(&sweep_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sweep"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<TVert>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &tattrs,
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Inst>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &iattrs,
                    }),
                ],
            },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: HDEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sweep"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: HEIGHT_FORMAT, blend: None, write_mask: wgpu::ColorWrites::RED })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // powierzchnia i boki: w głównym przebiegu widoku 3D
        let disp_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("millwise_gpu_disp_layout"),
            bind_group_layouts: &[Some(&disp_bgl)],
            immediate_size: 0,
        });
        let surface = |label: &str, vs: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&disp_layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    // jak siatki: linie leżące na powierzchni nie migoczą
                    bias: wgpu::DepthBiasState { constant: 2, slope_scale: 1.0, clamp: 0.0 },
                }),
                multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_surface"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let surface_pipeline = surface("millwise_gpu_surface", "vs_surface");
        let skirt_pipeline = surface("millwise_gpu_skirt", "vs_skirt");
        GpuSim {
            sweep_pipeline,
            surface_pipeline,
            skirt_pipeline,
            sweep_bgl,
            disp_bgl,
            sweep_ubuf,
            sweep_bg,
            disp_ubuf,
            partial_buf,
            built: None,
            drawn: 0,
            clear: true,
            last_time: f64::NEG_INFINITY,
            pending: (0, 0),
            partial: None,
        }
    }

    /// Buduje zasoby dla nowej symulacji: tekstura wysokości, wzorce narzędzi, instancje ruchów.
    fn build(&mut self, device: &wgpu::Device, input: &GpuSimInput) {
        let _ = &self.sweep_bgl;
        let [x0, y0, x1, y1] = input.area;
        let (w, h) = ((x1 - x0).max(1e-3), (y1 - y0).max(1e-3));
        let max_tex = device.limits().max_texture_dimension_2d.min(16384);
        let res = input.resolution.clamp(256, max_tex) as f64;
        let cell = w.max(h) / res;
        let tex = [((w / cell).ceil() as u32).clamp(2, max_tex), ((h / cell).ceil() as u32).clamp(2, max_tex)];
        let scale = (DISPLAY_MAX as f64 / tex[0].max(tex[1]) as f64).min(1.0);
        let grid = [((tex[0] as f64 * scale).round() as u32).max(2), ((tex[1] as f64 * scale).round() as u32).max(2)];
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("millwise_gpu_height"),
            size: wgpu::Extent3d { width: tex[0], height: tex[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HEIGHT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("millwise_gpu_height_depth"),
            size: wgpu::Extent3d { width: tex[0], height: tex[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&Default::default());
        let disp_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("millwise_gpu_disp_bg"),
            layout: &self.disp_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.disp_ubuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            ],
        });
        let templates = input
            .tools
            .iter()
            .map(|t| {
                let v = tool_template(t);
                let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("millwise_gpu_tool"),
                    contents: bytemuck::cast_slice(&v),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                (buf, v.len() as u32)
            })
            .collect();
        // ruchy robocze poniżej góry materiału (szybkie i nad materiałem nic nie zdejmują)
        let tl = input.timeline;
        let mut insts: Vec<Inst> = Vec::new();
        let mut cum = vec![0u32; tl.len().max(1)];
        let mut op_ranges: Vec<(usize, u32, u32)> = Vec::new();
        let f = |p: P3| [p.x as f32, p.y as f32, p.z as f32];
        for i in 1..tl.len() {
            let (a, b) = (tl[i - 1].p, tl[i].p);
            if !tl[i].rapid && a.z.min(b.z) < input.top && a.distance(b) > 1e-9 {
                let n = insts.len() as u32;
                match op_ranges.last_mut() {
                    Some(r) if r.0 == tl[i].op && r.2 == n => r.2 = n + 1,
                    _ => op_ranges.push((tl[i].op, n, n + 1)),
                }
                insts.push(Inst { a: f(a), b: f(b) });
            }
            cum[i] = insts.len() as u32;
        }
        let inst = insts
            .chunks(INST_CHUNK)
            .map(|part| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("millwise_gpu_moves"),
                    contents: bytemuck::cast_slice(part),
                    usage: wgpu::BufferUsages::VERTEX,
                })
            })
            .collect();
        // głębokość 0..1 = Z od dużo poniżej spodu (przelotowe) do góry materiału
        let zlo = input.bottom - 100.0;
        let size = [tex[0] as f64 * cell, tex[1] as f64 * cell];
        let sweep_u = SweepU { area: [x0 as f32, y0 as f32, size[0] as f32, size[1] as f32], z: [zlo as f32, input.top as f32, 0.0, 0.0] };
        self.built = Some(Built {
            key: (input.key, input.resolution),
            tex,
            grid,
            view,
            depth_view,
            disp_bg,
            templates,
            inst,
            op_ranges,
            cum,
            sweep_u,
            area: [x0, y0, x0 + size[0], y0 + size[1]],
            top: input.top,
            bottom: input.bottom,
        });
        self.drawn = 0;
        self.clear = true;
        self.last_time = f64::NEG_INFINITY;
    }

    /// Przygotowanie klatki: co dorysować w teksturze wysokości.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, input: &GpuSimInput) {
        if self.built.as_ref().is_none_or(|b| b.key != (input.key, input.resolution)) {
            self.build(device, input);
        }
        let b = self.built.as_ref().unwrap();
        queue.write_buffer(&self.sweep_ubuf, 0, bytemuck::bytes_of(&b.sweep_u));
        let tl = input.timeline;
        // cofnięcie (albo ruch częściowy dalej niż teraz) — od nowa
        if input.time < self.last_time {
            self.drawn = 0;
            self.clear = true;
        }
        self.last_time = input.time;
        let kk = tl.partition_point(|p| p.t <= input.time);
        let done = if kk == 0 { 0 } else { b.cum[(kk - 1).min(b.cum.len() - 1)] };
        self.pending = (self.drawn.min(done), done);
        if done < self.drawn {
            self.drawn = 0;
            self.clear = true;
            self.pending = (0, done);
        }
        // bieżący ruch do pozycji narzędzia
        self.partial = None;
        if kk >= 1 && kk < tl.len() {
            let (a, c) = (tl[kk - 1], tl[kk]);
            if !c.rapid && c.t > a.t && input.time > a.t && a.p.z.min(c.p.z) < input.top {
                let f = ((input.time - a.t) / (c.t - a.t)).clamp(0.0, 1.0);
                let q = P3::new(a.p.x + (c.p.x - a.p.x) * f, a.p.y + (c.p.y - a.p.y) * f, a.p.z + (c.p.z - a.p.z) * f);
                let inst = Inst { a: [a.p.x as f32, a.p.y as f32, a.p.z as f32], b: [q.x as f32, q.y as f32, q.z as f32] };
                queue.write_buffer(&self.partial_buf, 0, bytemuck::bytes_of(&inst));
                self.partial = Some(c.op);
            }
        }
    }

    /// Rysuje nowe ruchy do tekstury wysokości (przed głównym przebiegiem).
    pub fn encode(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let Some(b) = &self.built else { return };
        let (from, to) = self.pending;
        if !self.clear && from >= to && self.partial.is_none() {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("millwise_gpu_sweep_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &b.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // wysokość startowa = góra materiału
                    load: if self.clear {
                        wgpu::LoadOp::Clear(wgpu::Color { r: b.top, g: 0.0, b: 0.0, a: 1.0 })
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &b.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: if self.clear { wgpu::LoadOp::Clear(1.0) } else { wgpu::LoadOp::Load },
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.sweep_pipeline);
        pass.set_bind_group(0, &self.sweep_bg, &[]);
        for &(op, s, e) in &b.op_ranges {
            let (s, e) = (s.max(from), e.min(to));
            if s >= e {
                continue;
            }
            let Some((tbuf, tcount)) = b.templates.get(op) else { continue };
            pass.set_vertex_buffer(0, tbuf.slice(..));
            // w obrębie kawałków bufora instancji
            let mut k = s;
            while k < e {
                let chunk = k as usize / INST_CHUNK;
                let base = (chunk * INST_CHUNK) as u32;
                let end = e.min(base + INST_CHUNK as u32);
                pass.set_vertex_buffer(1, b.inst[chunk].slice(..));
                pass.draw(0..*tcount, (k - base)..(end - base));
                k = end;
            }
        }
        if let Some((tbuf, tcount)) = self.partial.and_then(|op| b.templates.get(op)) {
            pass.set_vertex_buffer(0, tbuf.slice(..));
            pass.set_vertex_buffer(1, self.partial_buf.slice(..));
            pass.draw(0..*tcount, 0..1);
        }
        drop(pass);
        self.drawn = to;
        self.clear = false;
    }

    /// Parametry wyświetlania (kamera i światło) przed głównym przebiegiem.
    pub fn write_uniforms(&self, queue: &wgpu::Queue, view_proj: Mat4, light: [f32; 4]) {
        let Some(b) = &self.built else { return };
        let u = DispU {
            view_proj: view_proj.to_cols_array_2d(),
            light,
            area: [b.area[0] as f32, b.area[1] as f32, (b.area[2] - b.area[0]) as f32, (b.area[3] - b.area[1]) as f32],
            dims: [b.tex[0] as f32, b.tex[1] as f32, b.grid[0] as f32, b.grid[1] as f32],
            zs: [b.sweep_u.z[0], b.sweep_u.z[1], b.bottom as f32, b.top as f32],
        };
        queue.write_buffer(&self.disp_ubuf, 0, bytemuck::bytes_of(&u));
    }

    /// Powierzchnia materiału, spód i boki — w głównym przebiegu widoku 3D.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(b) = &self.built else { return };
        let (gx, gy) = (b.grid[0], b.grid[1]);
        pass.set_bind_group(0, &b.disp_bg, &[]);
        pass.set_pipeline(&self.surface_pipeline);
        // instancja 0 = góra, 1 = spód
        pass.draw(0..(gx - 1) * (gy - 1) * 6, 0..2);
        pass.set_pipeline(&self.skirt_pipeline);
        pass.draw(0..(2 * (gx - 1) + 2 * (gy - 1)) * 6, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_templates_cover_the_tool() {
        for (kind, d) in [
            (cam_ops::ToolKind::FlatEnd, 6.0),
            (cam_ops::ToolKind::BallEnd, 6.0),
            (cam_ops::ToolKind::BullNose { corner_radius: 1.0 }, 6.0),
            (cam_ops::ToolKind::VBit { angle: 60.0, tip_diameter: 0.2 }, 3.0),
        ] {
            let t = ToolShape::new(&kind, d);
            let v = tool_template(&t);
            assert!(v.len() % 3 == 0 && !v.is_empty());
            // zasięg: promień narzędzia; najniżej czubek (z = 0)
            let rmax = v.iter().map(|q| (q.pos[0].powi(2) + q.pos[1].powi(2)).sqrt()).fold(0.0f32, f32::max);
            assert!((rmax - d as f32 / 2.0).abs() < 1e-4, "{kind:?}: {rmax}");
            let zmin = v.iter().map(|q| q.pos[2]).fold(f32::MAX, f32::min);
            assert!(zmin.abs() < 1e-6, "{kind:?}: {zmin}");
            // wysokość na obwodzie zgodna z profilem
            let edge = v.iter().filter(|q| ((q.pos[0].powi(2) + q.pos[1].powi(2)).sqrt() - d as f32 / 2.0).abs() < 1e-4).map(|q| q.pos[2]).fold(f32::MIN, f32::max);
            assert!((edge as f64 - t.profile(d / 2.0).unwrap_or(0.0)).abs() < 1e-4, "{kind:?}");
        }
    }
}
