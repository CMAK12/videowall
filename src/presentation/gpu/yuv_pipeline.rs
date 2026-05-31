//! YUV→RGB GPU pipeline.
//!
//! Owns the bind-group layout, sampler, render pipeline, and cached Y/U/V
//! input textures. The input textures are recreated when the frame
//! resolution changes (rare); the output texture is allocated fresh each
//! frame because `slint::Image::try_from(wgpu::Texture)` takes ownership.

use slint::wgpu_28::wgpu;

use crate::application::YuvFrame;

pub struct YuvPipeline {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    inputs: Option<InputTextures>,
}

struct InputTextures {
    width: u32,
    height: u32,
    y: wgpu::Texture,
    u: wgpu::Texture,
    v: wgpu::Texture,
    // Views are referenced by `bind_group` (wgpu retains them internally);
    // no need to store the TextureView handles here.
    bind_group: wgpu::BindGroup,
}

impl YuvPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("yuv_to_rgb"),
            source: wgpu::ShaderSource::Wgsl(include_str!("yuv_to_rgb.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("yuv_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                plane_binding(1),
                plane_binding(2),
                plane_binding(3),
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("yuv_pipeline_layout"),
            bind_group_layouts: &[&bgl],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("yuv_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("yuv_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            pipeline,
            bgl,
            sampler,
            inputs: None,
        }
    }

    /// Upload planes, run the YUV→RGB pass, and return the output RGBA8 texture.
    /// The returned texture is `RENDER_ATTACHMENT | TEXTURE_BINDING` so it can
    /// be handed to `slint::Image::try_from`.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &YuvFrame,
    ) -> wgpu::Texture {
        self.ensure_inputs(device, frame.width, frame.height);
        let inputs = self.inputs.as_ref().expect("inputs ensured above");

        upload_plane(
            queue,
            &inputs.y,
            &frame.y_plane,
            frame.y_stride,
            frame.height,
        );
        upload_plane(
            queue,
            &inputs.u,
            &frame.u_plane,
            frame.u_stride,
            frame.chroma_height(),
        );
        upload_plane(
            queue,
            &inputs.v,
            &frame.v_plane,
            frame.v_stride,
            frame.chroma_height(),
        );

        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("yuv_output"),
            size: wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("yuv_encoder"),
        });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("yuv_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    depth_slice: None,
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
            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &inputs.bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }
        queue.submit(Some(encoder.finish()));

        output
    }

    fn ensure_inputs(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if let Some(existing) = &self.inputs {
            if existing.width == width && existing.height == height {
                return;
            }
        }
        self.inputs = Some(InputTextures::new(
            device,
            &self.bgl,
            &self.sampler,
            width,
            height,
        ));
    }
}

impl InputTextures {
    fn new(
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> Self {
        let cw = width.div_ceil(2);
        let ch = height.div_ceil(2);

        let y = create_plane_texture(device, width, height, "y_plane");
        let u = create_plane_texture(device, cw, ch, "u_plane");
        let v = create_plane_texture(device, cw, ch, "v_plane");

        let y_view = y.create_view(&wgpu::TextureViewDescriptor::default());
        let u_view = u.create_view(&wgpu::TextureViewDescriptor::default());
        let v_view = v.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("yuv_bind_group"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&y_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&u_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&v_view),
                },
            ],
        });

        Self {
            width,
            height,
            y,
            u,
            v,
            bind_group,
        }
    }
}

fn plane_binding(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn create_plane_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    label: &str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn upload_plane(queue: &wgpu::Queue, texture: &wgpu::Texture, data: &[u8], stride: u32, rows: u32) {
    let size = texture.size();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(stride),
            rows_per_image: Some(rows),
        },
        wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
    );
}
