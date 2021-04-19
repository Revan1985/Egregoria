use crate::uiworld::UiWorld;
use common::{FastMap, Z_TERRAIN};
use geom::{vec2, LinearColor};
use std::mem::MaybeUninit;
use wgpu_engine::pbuffer::PBuffer;
use wgpu_engine::wgpu::BufferUsage;
use wgpu_engine::IndexType;
use wgpu_engine::{ColoredVertex, FrameContext, GfxContext, Mesh};

const CHUNK_SIZE: f32 = 1000.0;
const RESOLUTION: usize = 40;
const LOD: usize = 4;

struct TerrainChunk {
    lods: [Mesh; LOD],
}

pub struct TerrainRender {
    chunks: FastMap<(i32, i32), TerrainChunk>,
    indices: [(PBuffer, usize); LOD],
}

impl TerrainRender {
    pub fn new(gfx: &mut GfxContext) -> Self {
        let indices = Self::generate_indices(gfx);
        let mut me = TerrainRender {
            chunks: Default::default(),
            indices,
        };

        for y in -10..10 {
            for x in -10..10 {
                me.generate(gfx, x, y);
            }
        }
        me
    }

    fn generate(&mut self, gfx: &mut GfxContext, x: i32, y: i32) {
        let mut v = vec![];
        for lod in 0..LOD {
            let resolution = RESOLUTION / (1 << lod);

            let mut mesh = Vec::with_capacity((resolution + 1) * (resolution + 1));

            let offset = vec2(x as f32, y as f32) * CHUNK_SIZE;

            for y in 0..=resolution {
                let y = y as f32 / resolution as f32;
                for x in 0..=resolution {
                    let x = x as f32 / resolution as f32;
                    let pos = vec2(x, y);
                    let pos = pos * CHUNK_SIZE + offset;

                    let height = map_model::procgen::heightmap::height(pos);

                    let col: LinearColor = if height < 0.1 {
                        common::config().sea_col.into()
                    } else if height < 0.12 {
                        common::config().sand_col.into()
                    } else {
                        0.37 * LinearColor::from(common::config().grass_col)
                    };

                    mesh.push(ColoredVertex {
                        position: [pos.x, pos.y, Z_TERRAIN],
                        color: col.into(),
                    })
                }
            }

            let (ref indice, n_indices) = self.indices[lod];

            let mut vbuf = PBuffer::new(BufferUsage::VERTEX);
            vbuf.write(gfx, bytemuck::cast_slice(&mesh));
            let m = Mesh {
                vertex_buffer: vbuf.inner().expect("vertices were generated"),
                index_buffer: indice.inner().expect("indices was generated"),
                n_indices: n_indices as u32,
            };
            v.push(m);
        }

        let chunk = TerrainChunk {
            lods: collect_arr4(v),
        };
        self.chunks.insert((x, y), chunk);
    }

    fn generate_indices(gfx: &GfxContext) -> [(PBuffer, usize); 4] {
        let mut v = vec![];
        for lod in 0..LOD {
            let resolution = RESOLUTION / (1 << lod);
            let mut indices: Vec<IndexType> = Vec::with_capacity(6 * resolution * resolution);

            let w = (resolution + 1) as IndexType;
            for y in 0..resolution as IndexType {
                for x in 0..resolution as IndexType {
                    let idx = y * w + x;
                    indices.push(idx);
                    indices.push(idx + 1);
                    indices.push(idx + w + 1);

                    indices.push(idx);
                    indices.push(idx + w + 1);
                    indices.push(idx + w);
                }
            }

            let l = indices.len();

            let mut buf = PBuffer::new(BufferUsage::INDEX);
            buf.write(gfx, bytemuck::cast_slice(&indices));
            v.push((buf, l));
        }
        collect_arr4(v)
    }

    pub fn render(&mut self, _uiw: &UiWorld, fctx: &mut FrameContext) {
        for chunk in self.chunks.values() {
            fctx.objs.push(Box::new(chunk.lods[0].clone()))
        }
    }
}

fn collect_arr4<T>(x: impl IntoIterator<Item = T>) -> [T; LOD] {
    let mut arr = MaybeUninit::uninit();

    let mut ptr = arr.as_mut_ptr() as *mut T;
    let mut i = 0;
    for v in x {
        if i == LOD {
            panic!("not 4")
        }
        unsafe {
            ptr.write(v);
            ptr = ptr.add(1);
        }
        i += 1;
    }

    if i < LOD {
        panic!("not 4")
    }

    unsafe { arr.assume_init() }
}