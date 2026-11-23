use crate::DISPLAY_HEIGHT;

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct Target {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

impl Target {
    fn new(device: &wgpu::Device, label: &str, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
            size: (width.max(1), height.max(1)),
        }
    }
}

pub struct CrtFilter {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    beam_pipeline: wgpu::RenderPipeline,
    horizontal_pipeline: wgpu::RenderPipeline,
    vertical_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    beam: Target,
    horizontal: Target,
    halation: Target,
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    entry: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[Some(format.into())],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    target: &'a wgpu::TextureView,
    label: &str,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
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
    })
}

impl CrtFilter {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("crt.wgsl"));
        let texture_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("crt"),
            entries: &[
                texture_entry(0),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(3),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("crt"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("crt"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let build =
            |entry: &str, format| pipeline(device, &pipeline_layout, &shader, entry, format);
        Self {
            beam_pipeline: build("fs_beam", TARGET_FORMAT),
            horizontal_pipeline: build("fs_blur_horizontal", TARGET_FORMAT),
            vertical_pipeline: build("fs_blur_vertical", TARGET_FORMAT),
            composite_pipeline: build("fs_composite", surface_format),
            layout,
            sampler,
            beam: Target::new(device, "crt beam", 1, 1),
            horizontal: Target::new(device, "crt horizontal", 1, 1),
            halation: Target::new(device, "crt halation", 1, 1),
        }
    }

    fn bind(
        &self,
        device: &wgpu::Device,
        primary: &wgpu::TextureView,
        secondary: &wgpu::TextureView,
        params: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("crt"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(primary),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(secondary),
                },
            ],
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::TextureView,
        frame_width: u32,
        params: &wgpu::Buffer,
        target: &wgpu::TextureView,
        viewport: crate::Viewport,
    ) {
        let limit = device.limits().max_texture_dimension_2d;
        let beam_size = (viewport.width.min(limit), viewport.height.min(limit));
        if self.beam.size != beam_size {
            self.beam = Target::new(device, "crt beam", beam_size.0, beam_size.1);
        }
        if self.horizontal.size != (frame_width, DISPLAY_HEIGHT) {
            self.horizontal = Target::new(device, "crt horizontal", frame_width, DISPLAY_HEIGHT);
            self.halation = Target::new(device, "crt halation", frame_width, DISPLAY_HEIGHT);
        }
        let stages = [
            (
                &self.beam_pipeline,
                frame,
                frame,
                &self.beam.view,
                "crt beam",
            ),
            (
                &self.horizontal_pipeline,
                frame,
                frame,
                &self.horizontal.view,
                "crt horizontal",
            ),
            (
                &self.vertical_pipeline,
                &self.horizontal.view,
                &self.horizontal.view,
                &self.halation.view,
                "crt vertical",
            ),
        ];
        for (pipeline, primary, secondary, output, label) in stages {
            let group = self.bind(device, primary, secondary, params);
            let mut render = pass(encoder, output, label);
            render.set_pipeline(pipeline);
            render.set_bind_group(0, &group, &[]);
            render.draw(0..3, 0..1);
        }
        let group = self.bind(device, &self.beam.view, &self.halation.view, params);
        let mut render = pass(encoder, target, "crt composite");
        render.set_viewport(
            viewport.x as f32,
            viewport.y as f32,
            viewport.width as f32,
            viewport.height as f32,
            0.0,
            1.0,
        );
        render.set_pipeline(&self.composite_pipeline);
        render.set_bind_group(0, &group, &[]);
        render.draw(0..3, 0..1);
    }
}
