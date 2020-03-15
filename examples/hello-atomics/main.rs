use std::convert::TryInto as _;
use zerocopy::AsBytes as _;

// Example showing atomic operations on a texture.

async fn run() {
    env_logger::init();
    execute_gpu().await;
}

async fn execute_gpu() {
    let adapter = wgpu::Adapter::request(
        &wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::Default,
        },
        wgpu::BackendBit::PRIMARY,
    )
    .unwrap();

    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
        extensions: wgpu::Extensions {
            anisotropic_filtering: false,
        },
        limits: wgpu::Limits::default(),
    });

    let cs = include_bytes!("shader.comp.spv");
    let cs_module =
        device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(&cs[..])).unwrap());
    let texture_extent = wgpu::Extent3d {
        width: 1,
        height: 1,
        depth: 1,
    };
    let num_elements = texture_extent.width * texture_extent.height * texture_extent.depth;
    let empty_data: Vec<u32> = vec![0 as u32; num_elements as usize];
    let data_size = (num_elements as usize) * std::mem::size_of::<u32>();

    let staging_buffer = device.create_buffer_with_data(
        empty_data.as_slice().as_bytes(),
        wgpu::BufferUsage::MAP_READ | wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::COPY_SRC,
    );

    let density_texture = device.create_texture(&wgpu::TextureDescriptor {
        size: texture_extent,
        array_layer_count: 1,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsage::COPY_SRC
            | wgpu::TextureUsage::STORAGE
            | wgpu::TextureUsage::OUTPUT_ATTACHMENT
            | wgpu::TextureUsage::COPY_DST
            | wgpu::TextureUsage::SAMPLED,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStage::COMPUTE,
            ty: wgpu::BindingType::StorageTexture {
                dimension: wgpu::TextureViewDimension::D2,
                format: wgpu::TextureFormat::R32Uint,
                readonly: false,
            },
        }],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        bindings: &[wgpu::Binding {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&density_texture.create_default_view()),
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        layout: &pipeline_layout,
        compute_stage: wgpu::ProgrammableStageDescriptor {
            module: &cs_module,
            entry_point: "main",
        },
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });
    encoder.copy_buffer_to_texture(
        wgpu::BufferCopyView {
            buffer: &staging_buffer,
            offset: 0,
            bytes_per_row: std::mem::size_of::<u32>() as u32 * texture_extent.width,
            rows_per_image: texture_extent.height,
        },
        wgpu::TextureCopyView {
            texture: &density_texture,
            mip_level: 0,
            array_layer: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        texture_extent,
    );
    {
        let mut cpass = encoder.begin_compute_pass();
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch(500, 1, 1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TextureCopyView {
            texture: &density_texture,
            mip_level: 0,
            array_layer: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        wgpu::BufferCopyView {
            buffer: &staging_buffer,
            offset: 0,
            bytes_per_row: std::mem::size_of::<u32>() as u32 * texture_extent.width,
            rows_per_image: texture_extent.height,
        },
        texture_extent,
    );
    queue.submit(&[encoder.finish()]);
    if let Ok(mapping) = staging_buffer.map_read(0u64, data_size as u64).await {
        println!("Succeeded in mapping staging buffer.");
        for v in mapping
            .as_slice()
            .chunks_exact(4)
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
        {
            println!("{}", v);
        }
    } else {
        println!("Failed to get staging buffer.");
    }
    println!("Done!");
}

fn main() {
    futures::executor::block_on(run());
}
