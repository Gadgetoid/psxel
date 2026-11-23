use crate::crt::CrtFilter;
use crate::{bgr555_to_rgba8, DISPLAY_HEIGHT};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub fn letterbox(surface_width: u32, surface_height: u32) -> Viewport {
    let aspect_width = |height: u32| height * 4 / 3;
    let integer_scale =
        (surface_height / DISPLAY_HEIGHT).min(surface_width * 3 / 4 / DISPLAY_HEIGHT);
    let (width, height) = if integer_scale >= 1 {
        let height = DISPLAY_HEIGHT * integer_scale;
        (aspect_width(height), height)
    } else if surface_width * 3 >= surface_height * 4 {
        (aspect_width(surface_height), surface_height)
    } else {
        (surface_width, surface_width * 3 / 4)
    };
    Viewport {
        x: (surface_width - width) / 2,
        y: (surface_height - height) / 2,
        width: width.max(1),
        height: height.max(1),
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Params {
    viewport_origin: [f32; 2],
    viewport_size: [f32; 2],
    source_size: [f32; 2],
    effect: [f32; 2],
}

impl Params {
    fn as_bytes(&self) -> Vec<u8> {
        [
            self.viewport_origin,
            self.viewport_size,
            self.source_size,
            self.effect,
        ]
        .iter()
        .flatten()
        .flat_map(|value| value.to_le_bytes())
        .collect()
    }
}

pub struct Presenter {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    frame: wgpu::Texture,
    frame_width: u32,
    staging: Vec<u8>,
    afterglow: Vec<f32>,
    surface_format: wgpu::TextureFormat,
    crt: Option<CrtFilter>,
}

impl Presenter {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        frame_width: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("present.wgsl"));
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(surface_format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("present params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame = Self::create_frame(device, frame_width);
        let bind_group =
            Self::create_bind_group(device, &bind_group_layout, &frame, &params_buffer);
        Self {
            pipeline,
            bind_group_layout,
            bind_group,
            params_buffer,
            frame,
            frame_width,
            staging: Vec::new(),
            afterglow: Vec::new(),
            surface_format,
            crt: None,
        }
    }

    fn create_frame(device: &wgpu::Device, width: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame"),
            size: wgpu::Extent3d {
                width,
                height: DISPLAY_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        frame: &wgpu::Texture,
        params_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        let view = frame.create_view(&wgpu::TextureViewDescriptor::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        })
    }

    pub fn frame_width(&self) -> u32 {
        self.frame_width
    }

    pub fn set_frame_width(&mut self, device: &wgpu::Device, width: u32) {
        if width == self.frame_width {
            return;
        }
        self.frame_width = width;
        self.frame = Self::create_frame(device, width);
        self.bind_group = Self::create_bind_group(
            device,
            &self.bind_group_layout,
            &self.frame,
            &self.params_buffer,
        );
    }

    pub fn frame_view(&self) -> wgpu::TextureView {
        self.frame
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn upload_bgr555(
        &mut self,
        queue: &wgpu::Queue,
        pixels: &[u16],
        stride: usize,
        persistence: f32,
    ) {
        let width = self.frame_width as usize;
        let height = DISPLAY_HEIGHT as usize;
        self.staging.clear();
        for row in 0..height {
            for &pixel in &pixels[row * stride..row * stride + width] {
                self.staging.extend_from_slice(&bgr555_to_rgba8(pixel));
            }
        }
        if persistence > 0.0 {
            self.apply_afterglow(persistence);
        } else {
            self.afterglow.clear();
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.frame,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.staging,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.frame_width * 4),
                rows_per_image: Some(DISPLAY_HEIGHT),
            },
            self.frame.size(),
        );
    }

    fn apply_afterglow(&mut self, persistence: f32) {
        if self.afterglow.len() != self.staging.len() {
            self.afterglow = self
                .staging
                .iter()
                .map(|&value| (value as f32 / 255.0).powi(2))
                .collect();
            return;
        }
        for (index, value) in self.staging.iter_mut().enumerate() {
            if index % 4 == 3 {
                continue;
            }
            let fresh = (*value as f32 / 255.0).powi(2);
            let glow = self.afterglow[index] * persistence;
            let lit = fresh.max(glow);
            self.afterglow[index] = lit;
            *value = (lit.sqrt() * 255.0).round() as u8;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        surface_width: u32,
        surface_height: u32,
        crt: bool,
    ) {
        let viewport = letterbox(surface_width, surface_height);
        let params = Params {
            viewport_origin: [viewport.x as f32, viewport.y as f32],
            viewport_size: [viewport.width as f32, viewport.height as f32],
            source_size: [self.frame_width as f32, DISPLAY_HEIGHT as f32],
            effect: [0.0; 2],
        };
        queue.write_buffer(&self.params_buffer, 0, &params.as_bytes());
        if crt {
            let format = self.surface_format;
            let filter = self
                .crt
                .get_or_insert_with(|| CrtFilter::new(device, format));
            let frame = self
                .frame
                .create_view(&wgpu::TextureViewDescriptor::default());
            filter.render(
                device,
                encoder,
                &frame,
                self.frame_width,
                &self.params_buffer,
                target,
                viewport,
            );
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(
            viewport.x as f32,
            viewport.y as f32,
            viewport.width as f32,
            viewport.height as f32,
            0.0,
            1.0,
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_scale_when_it_fits() {
        assert_eq!(
            letterbox(1920, 1080),
            Viewport {
                x: 320,
                y: 60,
                width: 1280,
                height: 960
            }
        );
        assert_eq!(
            letterbox(320, 240),
            Viewport {
                x: 0,
                y: 0,
                width: 320,
                height: 240
            }
        );
    }

    #[test]
    fn pillarbox_on_wide_surfaces() {
        let viewport = letterbox(2000, 480);
        assert_eq!((viewport.width, viewport.height), (640, 480));
        assert_eq!(viewport.x, 680);
    }

    #[test]
    fn fit_below_native_size() {
        assert_eq!(
            letterbox(200, 400),
            Viewport {
                x: 0,
                y: 125,
                width: 200,
                height: 150
            }
        );
        assert_eq!(
            letterbox(400, 200),
            Viewport {
                x: 67,
                y: 0,
                width: 266,
                height: 200
            }
        );
    }
}
