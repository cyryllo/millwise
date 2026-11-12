//! Renderer 3D na wgpu: rysuje do tekstury poza ekranem (MSAA + bufor
//! głębokości), którą egui wyświetla jako obraz.

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;
use glam::Mat4;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct LineVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light: [f32; 4],
}

/// Geometria sceny przygotowana na CPU.
#[derive(Default)]
pub struct SceneData {
    pub lines: Vec<LineVertex>,
    pub opaque: Vec<MeshVertex>,
    pub transparent: Vec<MeshVertex>,
}

struct GpuBuf {
    buf: wgpu::Buffer,
    count: u32,
}

/// Bufor wielokrotnego użytku (zapis przez queue.write_buffer, gdy rozmiar wystarcza).
struct DynBuf {
    buf: wgpu::Buffer,
    capacity: u64,
    count: u32,
}

impl DynBuf {
    fn write(slot: &mut Option<DynBuf>, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8], count: u32, usage: wgpu::BufferUsages, label: &str) {
        if count == 0 {
            if let Some(b) = slot {
                b.count = 0;
            }
            return;
        }
        let size = data.len() as u64;
        match slot {
            Some(b) if b.capacity >= size => {
                queue.write_buffer(&b.buf, 0, data);
                b.count = count;
            }
            _ => {
                let capacity = (size * 5 / 4).max(1024).next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT);
                let buf = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: capacity,
                    usage: usage | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                queue.write_buffer(&buf, 0, data);
                *slot = Some(DynBuf { buf, capacity, count });
            }
        }
    }
}

struct Target {
    size: [u32; 2],
    msaa_view: wgpu::TextureView,
    resolve_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

pub struct Renderer {
    line_pipeline: wgpu::RenderPipeline,
    mesh_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    lines: Option<GpuBuf>,
    opaque: Option<GpuBuf>,
    transparent: Option<GpuBuf>,
    sim_vb: Option<DynBuf>,
    sim_ib: Option<DynBuf>,
    dyn_lines: Option<DynBuf>,
    target: Option<Target>,
    texture_id: Option<eframe::egui::TextureId>,
    pub clear_color: wgpu::Color,
}

impl Renderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("millwise_3d"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("millwise_3d_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("millwise_3d_uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("millwise_3d_bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("millwise_3d_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let line_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4];
        let line_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &line_attrs,
        };
        let mesh_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &mesh_attrs,
        };

        let make = |label: &str,
                    vs: &str,
                    fs: &str,
                    vlayout: &wgpu::VertexBufferLayout,
                    topology: wgpu::PrimitiveTopology,
                    depth_write: bool,
                    bias: i32| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[Some(vlayout.clone())],
                },
                primitive: wgpu::PrimitiveState { topology, cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(depth_write),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState { constant: bias, slope_scale: if bias != 0 { 1.0 } else { 0.0 }, clamp: 0.0 },
                }),
                multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
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

        // Siatki dostają dodatni bias głębokości, żeby linie leżące na
        // powierzchniach (krawędzie, kontury) nie migotały.
        let line_pipeline =
            make("millwise_lines", "vs_line", "fs_line", &line_layout, wgpu::PrimitiveTopology::LineList, true, 0);
        let mesh_pipeline =
            make("millwise_mesh", "vs_mesh", "fs_mesh", &mesh_layout, wgpu::PrimitiveTopology::TriangleList, true, 2);
        let transparent_pipeline = make(
            "millwise_mesh_transparent",
            "vs_mesh",
            "fs_mesh",
            &mesh_layout,
            wgpu::PrimitiveTopology::TriangleList,
            false,
            2,
        );

        Self {
            line_pipeline,
            mesh_pipeline,
            transparent_pipeline,
            uniform_buf,
            bind_group,
            lines: None,
            opaque: None,
            transparent: None,
            sim_vb: None,
            sim_ib: None,
            dyn_lines: None,
            target: None,
            texture_id: None,
            clear_color: wgpu::Color { r: 0.012, g: 0.014, b: 0.018, a: 1.0 },
        }
    }

    pub fn upload(&mut self, device: &wgpu::Device, scene: &SceneData) {
        fn mk<T: Pod>(device: &wgpu::Device, label: &str, data: &[T]) -> Option<GpuBuf> {
            if data.is_empty() {
                return None;
            }
            let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(data),
                usage: wgpu::BufferUsages::VERTEX,
            });
            Some(GpuBuf { buf, count: data.len() as u32 })
        }
        self.lines = mk(device, "millwise_lines_vb", &scene.lines);
        self.opaque = mk(device, "millwise_opaque_vb", &scene.opaque);
        self.transparent = mk(device, "millwise_transparent_vb", &scene.transparent);
    }

    /// Siatka materiału z symulacji (indeksowana); pusta = ukryta.
    pub fn upload_sim(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, verts: &[MeshVertex], indices: &[u32]) {
        let idx = indices;
        DynBuf::write(&mut self.sim_vb, device, queue, bytemuck::cast_slice(verts), verts.len() as u32, wgpu::BufferUsages::VERTEX, "millwise_sim_vb");
        DynBuf::write(&mut self.sim_ib, device, queue, bytemuck::cast_slice(idx), idx.len() as u32, wgpu::BufferUsages::INDEX, "millwise_sim_ib");
    }

    /// Linie zmieniające się co klatkę (narzędzie).
    pub fn upload_dynamic_lines(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, lines: &[LineVertex]) {
        DynBuf::write(&mut self.dyn_lines, device, queue, bytemuck::cast_slice(lines), lines.len() as u32, wgpu::BufferUsages::VERTEX, "millwise_dyn_lines");
    }

    fn ensure_target(&mut self, rs: &egui_wgpu::RenderState, size: [u32; 2]) {
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let device = &rs.device;
        let extent = wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 };
        let tex = |label: &str, format, samples, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let msaa = tex("millwise_msaa", COLOR_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let resolve = tex(
            "millwise_resolve",
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = tex("millwise_depth", DEPTH_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let target = Target {
            size,
            msaa_view: msaa.create_view(&Default::default()),
            resolve_view: resolve.create_view(&Default::default()),
            depth_view: depth.create_view(&Default::default()),
        };
        let mut egui_renderer = rs.renderer.write();
        match self.texture_id {
            Some(id) => egui_renderer.update_egui_texture_from_wgpu_texture(
                device,
                &target.resolve_view,
                wgpu::FilterMode::Linear,
                id,
            ),
            None => {
                self.texture_id =
                    Some(egui_renderer.register_native_texture(device, &target.resolve_view, wgpu::FilterMode::Linear))
            }
        }
        self.target = Some(target);
    }

    /// Renderuje scenę i zwraca teksturę egui z wynikiem.
    pub fn render(&mut self, rs: &egui_wgpu::RenderState, size: [u32; 2], view_proj: Mat4) -> eframe::egui::TextureId {
        let size = [size[0].max(1), size[1].max(1)];
        self.ensure_target(rs, size);
        let u = Uniforms { view_proj: view_proj.to_cols_array_2d(), light: [0.35, -0.5, 0.8, 0.35] };
        rs.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&u));

        let target = self.target.as_ref().unwrap();
        let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("millwise_3d") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("millwise_3d_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.msaa_view,
                    depth_slice: None,
                    resolve_target: Some(&target.resolve_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(self.clear_color), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            if let Some(b) = &self.opaque {
                pass.set_pipeline(&self.mesh_pipeline);
                pass.set_vertex_buffer(0, b.buf.slice(..));
                pass.draw(0..b.count, 0..1);
            }
            if let (Some(vb), Some(ib)) = (&self.sim_vb, &self.sim_ib) {
                if vb.count > 0 && ib.count > 0 {
                    pass.set_pipeline(&self.mesh_pipeline);
                    pass.set_vertex_buffer(0, vb.buf.slice(..));
                    pass.set_index_buffer(ib.buf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..ib.count, 0, 0..1);
                }
            }
            if let Some(b) = &self.lines {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_vertex_buffer(0, b.buf.slice(..));
                pass.draw(0..b.count, 0..1);
            }
            if let Some(b) = self.dyn_lines.as_ref().filter(|b| b.count > 0) {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_vertex_buffer(0, b.buf.slice(..));
                pass.draw(0..b.count, 0..1);
            }
            if let Some(b) = &self.transparent {
                pass.set_pipeline(&self.transparent_pipeline);
                pass.set_vertex_buffer(0, b.buf.slice(..));
                pass.draw(0..b.count, 0..1);
            }
        }
        rs.queue.submit(Some(encoder.finish()));
        self.texture_id.unwrap()
    }
}
