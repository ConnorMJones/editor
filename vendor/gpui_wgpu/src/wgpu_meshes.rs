use std::sync::Arc;

use collections::FxHashMap;
use gpui::{MeshData, MeshKey, MeshVersion, PlatformMeshes, Scene};
use parking_lot::Mutex;

use crate::WgpuContext;

/// Meshes kept on the GPU; uploads wait for the next frame, and a frame frees the meshes its
/// scene no longer draws.
pub struct WgpuMeshes(Mutex<MeshesState>);

struct MeshesState {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    resident: FxHashMap<MeshKey, GpuMesh>,
    pending: FxHashMap<MeshKey, Pending>,
    /// Counts frames, to stamp the meshes each one draws.
    frame: u64,
}

pub(crate) struct GpuMesh {
    version: MeshVersion,
    drawn: u64,
    pub(crate) vertices: wgpu::Buffer,
    pub(crate) indices: wgpu::Buffer,
    pub(crate) index_count: u32,
}

struct Pending {
    version: MeshVersion,
    vertices: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl WgpuMeshes {
    pub fn from_context(context: &WgpuContext) -> Self {
        Self(Mutex::new(MeshesState {
            device: context.device.clone(),
            queue: context.queue.clone(),
            resident: FxHashMap::default(),
            pending: FxHashMap::default(),
            frame: 0,
        }))
    }

    /// Writes the queued uploads, into a mesh's existing buffers where they fit.
    pub(crate) fn before_frame(&self) {
        let mut state = self.0.lock();
        let MeshesState {
            device,
            queue,
            resident,
            pending,
            frame,
        } = &mut *state;
        for (key, upload) in pending.drain() {
            let vertex_bytes: &[u8] = bytemuck::cast_slice(&upload.vertices);
            let index_bytes: &[u8] = bytemuck::cast_slice(&upload.indices);
            match resident.get_mut(&key) {
                Some(mesh)
                    if mesh.vertices.size() >= vertex_bytes.len() as u64
                        && mesh.indices.size() >= index_bytes.len() as u64 =>
                {
                    queue.write_buffer(&mesh.vertices, 0, vertex_bytes);
                    queue.write_buffer(&mesh.indices, 0, index_bytes);
                    mesh.version = upload.version;
                    mesh.index_count = upload.indices.len() as u32;
                }
                Some(_) | None => {
                    let buffer = |label: &str, usage: wgpu::BufferUsages, bytes: &[u8]| {
                        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some(label),
                            size: (bytes.len() as u64).next_multiple_of(4).max(4),
                            usage: usage | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        queue.write_buffer(&buffer, 0, bytes);
                        buffer
                    };
                    resident.insert(
                        key,
                        GpuMesh {
                            version: upload.version,
                            drawn: *frame,
                            vertices: buffer(
                                "mesh_vertices",
                                wgpu::BufferUsages::VERTEX,
                                vertex_bytes,
                            ),
                            indices: buffer("mesh_indices", wgpu::BufferUsages::INDEX, index_bytes),
                            index_count: upload.indices.len() as u32,
                        },
                    );
                }
            }
        }
    }

    /// Runs `draw` with the resident meshes, which a scene's mesh keys look up.
    pub(crate) fn with_resident<R>(
        &self,
        draw: impl FnOnce(&FxHashMap<MeshKey, GpuMesh>) -> R,
    ) -> R {
        draw(&self.0.lock().resident)
    }

    /// Frees the meshes `scene` didn't draw.
    pub(crate) fn after_frame(&self, scene: &Scene) {
        let mut state = self.0.lock();
        state.frame += 1;
        let frame = state.frame;
        for mesh in &scene.meshes {
            if let Some(resident) = state.resident.get_mut(&mesh.key) {
                resident.drawn = frame;
            }
        }
        state.resident.retain(|_, mesh| mesh.drawn == frame);
    }

    pub fn clear(&self) {
        let mut state = self.0.lock();
        state.resident.clear();
        state.pending.clear();
    }

    /// Drops every mesh, which painting uploads again to the new device.
    pub fn handle_device_lost(&self, context: &WgpuContext) {
        let mut state = self.0.lock();
        state.device = context.device.clone();
        state.queue = context.queue.clone();
        state.resident.clear();
        state.pending.clear();
    }
}

impl PlatformMeshes for WgpuMeshes {
    fn ensure<'a>(
        &self,
        key: MeshKey,
        version: MeshVersion,
        data: &mut dyn FnMut() -> MeshData<'a>,
    ) {
        let mut state = self.0.lock();
        let current = (state.resident.get(&key).map(|mesh| mesh.version))
            .into_iter()
            .chain(state.pending.get(&key).map(|upload| upload.version))
            .last();
        match current == Some(version) {
            true => {}
            false => {
                let data = data();
                state.pending.insert(
                    key,
                    Pending {
                        version,
                        vertices: data.vertices.into_owned(),
                        indices: data.indices.into_owned(),
                    },
                );
            }
        }
    }
}
