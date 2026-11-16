//! Ścieżki narzędzia w widoku 2D rysowane przez kartę graficzną.
//!
//! Odcinki trafiają na kartę raz — po przeliczeniu ścieżek albo zmianie
//! zaznaczonej operacji — a każda klatka (przesuwanie, powiększanie) to tylko
//! nowa macierz widoku. Wynik jest rysowany do przezroczystej tekstury, którą
//! widok 2D nakłada na rysunek w miejscu dawnych linii z procesora.

use bytemuck::{Pod, Zeroable};
use eframe::egui::{self, Color32};
use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// Odcinków w jednym buforze (40 B każdy → 40 MB).
const CHUNK: usize = 1 << 20;

/// Rodzaj odcinka (zgodny z `params.y` w shaderze).
#[derive(Clone, Copy)]
pub enum SegKind {
    /// Linia o szerokości w punktach ekranu.
    Line,
    /// Pas o szerokości w mm (szerokość freza).
    Band,
    /// Przerywana linia w punktach ekranu (ruchy szybkie).
    Dashed,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Segment {
    seg: [f32; 4],
    color: [f32; 4],
    params: [f32; 2],
}

impl Segment {
    pub fn new(a: cam_geom::Vec2, b: cam_geom::Vec2, color: Color32, width: f32, kind: SegKind) -> Self {
        let mode = match kind {
            SegKind::Line => 0.0,
            SegKind::Band => 1.0,
            SegKind::Dashed => 2.0,
        };
        Segment {
            seg: [a.x as f32, a.y as f32, b.x as f32, b.y as f32],
            color: egui::Rgba::from(color).to_array(),
            params: [width, mode],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct Uniforms {
    center: [f32; 2],
    scale: f32,
    ppp: f32,
    size: [f32; 2],
    _pad: [f32; 2],
}

struct Target {
    size: [u32; 2],
    view: wgpu::TextureView,
}

pub struct PathsRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    chunks: Vec<(wgpu::Buffer, u32)>,
    target: Option<Target>,
    texture_id: Option<egui::TextureId>,
    /// Stan ostatniego rysowania — bez zmian tekstura zostaje ta sama.
    drawn: Option<(Uniforms, u64)>,
    generation: u64,
}

impl PathsRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("millwise_paths2d"),
            source: wgpu::ShaderSource::Wgsl(include_str!("paths2d.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("millwise_paths2d_bgl"),
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
            label: Some("millwise_paths2d_uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("millwise_paths2d_bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("millwise_paths2d_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x2];
        // mieszanie „max”: nakładające się odcinki dają ten sam kolor co jeden
        let max = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Max,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("millwise_paths2d"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Segment>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attrs,
                })],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: Some(wgpu::BlendState { color: max, alpha: max }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            uniform_buf,
            bind_group,
            chunks: Vec::new(),
            target: None,
            texture_id: None,
            drawn: None,
            generation: 0,
        }
    }

    /// Nowe odcinki (zastępują poprzednie).
    pub fn upload(&mut self, device: &wgpu::Device, segs: &[Segment]) {
        self.chunks = segs
            .chunks(CHUNK)
            .map(|c| {
                let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("millwise_paths2d_segments"),
                    contents: bytemuck::cast_slice(c),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                (buf, c.len() as u32)
            })
            .collect();
        self.generation += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    fn ensure_target(&mut self, rs: &egui_wgpu::RenderState, size: [u32; 2]) {
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let tex = rs.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("millwise_paths2d_target"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let mut egui_renderer = rs.renderer.write();
        match self.texture_id {
            Some(id) => egui_renderer.update_egui_texture_from_wgpu_texture(&rs.device, &view, wgpu::FilterMode::Nearest, id),
            None => {
                self.texture_id = Some(egui_renderer.register_native_texture(&rs.device, &view, wgpu::FilterMode::Nearest))
            }
        }
        self.target = Some(Target { size, view });
        self.drawn = None;
    }

    /// Rysuje odcinki dla widoku o środku `center` [mm] i skali `scale` [punkty/mm]
    /// do tekstury `size` [px] i zwraca ją. Bez zmian od ostatniej klatki — nic nie liczy.
    pub fn render(
        &mut self,
        rs: &egui_wgpu::RenderState,
        size: [u32; 2],
        center: cam_geom::Vec2,
        scale: f64,
        ppp: f32,
    ) -> egui::TextureId {
        let size = [size[0].max(1), size[1].max(1)];
        self.ensure_target(rs, size);
        let u = Uniforms {
            center: [center.x as f32, center.y as f32],
            scale: (scale * ppp as f64) as f32,
            ppp,
            size: [size[0] as f32, size[1] as f32],
            _pad: [0.0; 2],
        };
        if self.drawn != Some((u, self.generation)) {
            rs.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&u));
            let target = self.target.as_ref().unwrap();
            let mut encoder =
                rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("millwise_paths2d") });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("millwise_paths2d_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                for (buf, n) in &self.chunks {
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..6, 0..*n);
                }
            }
            rs.queue.submit(Some(encoder.finish()));
            self.drawn = Some((u, self.generation));
        }
        self.texture_id.unwrap()
    }
}
