use crate::pbuffer::PBuffer;
use crate::{
    compile_shader, Drawable, GfxContext, IndexType, MeshVertex, RenderParams, Texture, Uniform,
    VBDesc,
};
use std::sync::Arc;
use wgpu::{BindGroupLayout, BindGroupLayoutEntry, BufferUsage, Device, IndexFormat, RenderPass};

pub struct MeshBuilder {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<IndexType>,
    pub vbuffer: PBuffer,
    pub ibuffer: PBuffer,
}

impl Default for MeshBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self {
            vertices: vec![],
            indices: vec![],
            vbuffer: PBuffer::new(BufferUsage::VERTEX),
            ibuffer: PBuffer::new(BufferUsage::INDEX),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn extend(&mut self, vertices: &[MeshVertex], indices: &[IndexType]) -> &mut Self {
        let offset = self.vertices.len() as IndexType;
        self.vertices.extend_from_slice(vertices);
        self.indices.extend(indices.iter().map(|x| x + offset));
        self
    }

    #[inline(always)]
    pub fn extend_with(&mut self, f: impl FnOnce(&mut Vec<MeshVertex>, &mut dyn FnMut(IndexType))) {
        let offset = self.vertices.len() as IndexType;
        let vertices = &mut self.vertices;
        let indices = &mut self.indices;
        let mut x = move |index: IndexType| {
            indices.push(index + offset);
        };
        f(vertices, &mut x);
    }

    pub fn build(&mut self, gfx: &GfxContext, albedo: Arc<Texture>) -> Option<Mesh> {
        if self.vertices.is_empty() {
            return None;
        }

        self.vbuffer
            .write(gfx, bytemuck::cast_slice(&self.vertices));
        self.ibuffer.write(gfx, bytemuck::cast_slice(&self.indices));

        Some(Mesh {
            vertex_buffer: self.vbuffer.inner()?,
            index_buffer: self.ibuffer.inner()?,
            albedo_bg: Arc::new(
                albedo.bindgroup(&gfx.device, &Texture::bindgroup_layout(&gfx.device)),
            ),
            albedo,
            n_indices: self.indices.len() as u32,
            translucent: false,
        })
    }
}

#[derive(Clone)]
pub struct Mesh {
    pub vertex_buffer: Arc<wgpu::Buffer>,
    pub index_buffer: Arc<wgpu::Buffer>,
    pub albedo: Arc<Texture>,
    pub albedo_bg: Arc<wgpu::BindGroup>,
    pub n_indices: u32,
    pub translucent: bool,
}

impl Mesh {
    pub fn setup(gfx: &mut GfxContext) {
        let vert = compile_shader(&gfx.device, "assets/shaders/lit_mesh.vert", None);
        let frag = compile_shader(&gfx.device, "assets/shaders/pixel.frag", None);

        let pipe = gfx.color_pipeline(
            &[
                &gfx.projection.layout,
                &Uniform::<RenderParams>::bindgroup_layout(&gfx.device),
                &Texture::bindgroup_layout(&gfx.device),
                &bg_layout_litmesh(&gfx.device),
            ],
            &[MeshVertex::desc()],
            &vert,
            &frag,
        );
        gfx.register_pipeline::<Self>(pipe);

        gfx.register_pipeline::<LitMeshDepth>(gfx.depth_pipeline(
            &[MeshVertex::desc()],
            &vert,
            false,
        ));

        gfx.register_pipeline::<LitMeshDepthSMap>(gfx.depth_pipeline(
            &[MeshVertex::desc()],
            &vert,
            true,
        ));
    }
}

impl Drawable for Mesh {
    fn draw<'a>(&'a self, gfx: &'a GfxContext, rp: &mut RenderPass<'a>) {
        rp.set_pipeline(gfx.get_pipeline::<Self>());
        rp.set_bind_group(0, &gfx.projection.bindgroup, &[]);
        rp.set_bind_group(1, &gfx.render_params.bindgroup, &[]);
        rp.set_bind_group(2, &self.albedo_bg, &[]);
        rp.set_bind_group(3, &gfx.simplelit_bg, &[]);
        rp.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rp.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint32);
        rp.draw_indexed(0..self.n_indices, 0, 0..1);
    }

    fn draw_depth<'a>(
        &'a self,
        gfx: &'a GfxContext,
        rp: &mut RenderPass<'a>,
        shadow_map: bool,
        proj: &'a wgpu::BindGroup,
    ) {
        if self.translucent {
            return;
        }
        if shadow_map {
            rp.set_pipeline(gfx.get_pipeline::<LitMeshDepthSMap>());
        } else {
            rp.set_pipeline(gfx.get_pipeline::<LitMeshDepth>());
        }

        rp.set_bind_group(0, proj, &[]);
        rp.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rp.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint32);
        rp.draw_indexed(0..self.n_indices, 0, 0..1);
    }
}

struct LitMeshDepth;
struct LitMeshDepthSMap;

pub fn bg_layout_litmesh(device: &Device) -> BindGroupLayout {
    let entries: Vec<BindGroupLayoutEntry> = (0..4)
        .flat_map(|i| {
            vec![
                wgpu::BindGroupLayoutEntry {
                    binding: i * 2,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: if i != 2 {
                            wgpu::TextureSampleType::Float { filterable: true }
                        } else {
                            wgpu::TextureSampleType::Depth
                        },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: i * 2 + 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler {
                        filtering: true,
                        comparison: i == 2,
                    },
                    count: None,
                },
            ]
            .into_iter()
        })
        .collect::<Vec<_>>();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &entries,
        label: Some("Texture bindgroup layout"),
    })
}
