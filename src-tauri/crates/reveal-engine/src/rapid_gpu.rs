//! GPU execution of the Rapid engine's per-pixel pass (rapid.wgsl).
//!
//! The CPU path in `rapid.rs` stays the reference implementation and the
//! fallback: anything that fails here — no adapter, no device, a shader that
//! won't compile, a buffer that won't map — returns `None` and the caller
//! runs the CPU loop instead. A photo never fails to develop because of the
//! GPU; it just develops slower.
//!
//! What runs here is only the per-pixel stage. The ÷8 guidance map, grain,
//! and the LUT stacks stay on the CPU: the guidance map is 1/64 of the
//! pixels, and grain is stochastic. This is where the time actually goes —
//! it's the stage that forced the drag-resolution proxy in the first place.

use std::sync::OnceLock;

use spektrafilm_math::image::ImageBuf;
use wgpu::util::DeviceExt;

use crate::Recipe;

/// Mirrors `Params` in rapid.wgsl field for field. Scalars only plus three
/// trailing vec4s — no `vec3` and no arrays, because a uniform block gives
/// `vec3` 16-byte alignment and `array<f32, N>` a 16-byte STRIDE, either of
/// which would silently shift every field after it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    width: u32,
    height: u32,
    down_w: u32,
    down_h: u32,

    has_blurred: u32,
    has_hsl: u32,
    has_color_wheels: u32,
    has_zones: u32,

    w_mult: f32,
    exposure_factor: f32,
    r_temp: f32,
    r_tint: f32,

    g_tint: f32,
    b_temp: f32,
    b_tint: f32,
    brightness_adj: f32,

    clarity: f32,
    structure: f32,
    dehaze: f32,
    contrast: f32,

    shadows: f32,
    blacks: f32,
    highlights: f32,
    saturation_adj: f32,

    vibrance: f32,
    vignette_amount: f32,
    vignette_midpoint: f32,
    vignette_roundness: f32,

    vignette_feather: f32,
    highlight_desat: f32,
    use_logc: u32,
    agx_look: u32,

    zone_shadows_exposure: f32,
    zone_shadows_contrast: f32,
    zone_shadows_saturation: f32,
    zone_midtones_exposure: f32,

    zone_midtones_contrast: f32,
    zone_midtones_saturation: f32,
    zone_highlights_exposure: f32,
    zone_highlights_contrast: f32,

    zone_highlights_saturation: f32,
    curve_luma: u32,
    curve_r: u32,
    curve_g: u32,

    curve_b: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,

    shadows_tint: [f32; 4],
    midtones_tint: [f32; 4],
    highlights_tint: [f32; 4],
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

/// One device for the process, built on first use. Adapter + device request
/// costs tens of milliseconds — paying that per preview would cancel out
/// what the GPU is here to save.
static GPU: OnceLock<Option<Gpu>> = OnceLock::new();

fn gpu() -> Option<&'static Gpu> {
    GPU.get_or_init(|| {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))?;
        // downlevel_defaults() caps a storage binding at 128 MiB. At 12
        // bytes a pixel (3 × f32) that's ~11 MP — under a 40 MP export from
        // any modern body, which would fail validation and silently fall
        // back to the CPU exactly where the GPU is worth the most. Raise the
        // buffer limits to whatever this adapter actually offers.
        let adapter_limits = adapter.limits();
        let mut limits = wgpu::Limits::downlevel_defaults();
        limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
        limits.max_buffer_size = adapter_limits.max_buffer_size;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("reveal-rapid"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .ok()?;

        // A shader error must not take the app down with it — it has to
        // surface as "GPU unavailable" so the CPU path picks up.
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rapid.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("rapid.wgsl").into()),
        });

        let storage = |read_only: bool| wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let entry = |binding: u32, ty: wgpu::BindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rapid-bind-layout"),
            entries: &[
                entry(0, storage(true)),
                entry(1, storage(false)),
                entry(2, storage(true)),
                entry(
                    3,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(4, storage(true)),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rapid-pipeline-layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("rapid-pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        if let Some(err) = pollster::block_on(device.pop_error_scope()) {
            eprintln!("rapid gpu: shader/pipeline rejected, falling back to CPU: {err}");
            return None;
        }

        Some(Gpu { device, queue, pipeline, layout })
    })
    .as_ref()
}

/// True when a GPU is available at all — lets the caller skip building GPU
/// inputs it would only throw away.
pub fn available() -> bool {
    gpu().is_some()
}

/// Whether the GPU path should be used for this run. `REVEAL_NO_GPU=1`
/// forces the CPU loop — the escape hatch for a machine where a driver
/// renders wrong rather than failing outright, which no amount of error
/// scoping can catch from in here.
pub fn enabled() -> bool {
    static FORCED_OFF: OnceLock<bool> = OnceLock::new();
    let off = *FORCED_OFF.get_or_init(|| {
        std::env::var("REVEAL_NO_GPU").is_ok_and(|v| v != "0" && !v.is_empty())
    });
    !off && available()
}

/// Everything the shader needs that isn't derivable from the `Recipe`, so
/// the CPU side stays the single place these are computed.
pub struct Inputs<'a> {
    pub width: usize,
    pub height: usize,
    /// Scene-linear RGB, already through the pre-LUT stack.
    pub data: &'a [f32],
    /// The ÷8 gamma-encoded guidance map; empty when no stage needs it.
    pub blurred: &'a [f32],
    pub down_w: usize,
    pub down_h: usize,
    pub w_mult: f32,
    pub exposure_factor: f32,
    pub r_temp: f32,
    pub r_tint: f32,
    pub g_tint: f32,
    pub b_temp: f32,
    pub b_tint: f32,
    pub brightness_adj: f32,
    pub saturation_adj: f32,
    // develop_rapid rescales most tone sliders (÷100) into locals before the
    // pixel loop. They're carried here rather than re-read from the Recipe
    // so that scaling lives in exactly one place — reading `recipe.clarity`
    // here instead once made the GPU path 100x too strong, which is the kind
    // of drift the CPU/GPU equivalence test exists to catch.
    pub contrast: f32,
    pub shadows: f32,
    pub blacks: f32,
    pub highlights: f32,
    pub clarity: f32,
    pub structure: f32,
    pub dehaze: f32,
    pub vibrance: f32,
    pub has_hsl: bool,
    pub has_color_wheels: bool,
    pub has_zones: bool,
    /// The four tone-curve LUTs, `None` where the curve is identity.
    pub curves: [Option<Vec<f32>>; 4],
}

/// Run the per-pixel stage on the GPU. `None` means "couldn't" — never
/// "produced nothing"; the caller runs the CPU loop on `None`.
pub fn run(inputs: &Inputs, recipe: &Recipe) -> Option<ImageBuf> {
    let g = gpu()?;
    let total = inputs.width * inputs.height * 3;
    if total == 0 || inputs.data.len() < total {
        return None;
    }
    // Past the device's own ceiling there's nothing to do but hand it back
    // to the CPU — better a slow render than a validation failure per frame.
    let bytes = (total * std::mem::size_of::<f32>()) as u64;
    let limits = g.device.limits();
    if bytes > limits.max_storage_buffer_binding_size as u64 || bytes > limits.max_buffer_size {
        return None;
    }

    let agx_look = match recipe.agx_look.as_str() {
        "punchy" => 1u32,
        "golden" => 2,
        "soft" => 3,
        "bw" => 4,
        _ => 0,
    };
    let tint4 = |t: [f32; 3]| [t[0], t[1], t[2], 0.0];

    let params = Params {
        width: inputs.width as u32,
        height: inputs.height as u32,
        down_w: inputs.down_w.max(1) as u32,
        down_h: inputs.down_h.max(1) as u32,
        has_blurred: u32::from(!inputs.blurred.is_empty()),
        has_hsl: u32::from(inputs.has_hsl),
        has_color_wheels: u32::from(inputs.has_color_wheels),
        has_zones: u32::from(inputs.has_zones),
        w_mult: inputs.w_mult,
        exposure_factor: inputs.exposure_factor,
        r_temp: inputs.r_temp,
        r_tint: inputs.r_tint,
        g_tint: inputs.g_tint,
        b_temp: inputs.b_temp,
        b_tint: inputs.b_tint,
        brightness_adj: inputs.brightness_adj,
        clarity: inputs.clarity,
        structure: inputs.structure,
        dehaze: inputs.dehaze,
        contrast: inputs.contrast,
        shadows: inputs.shadows,
        blacks: inputs.blacks,
        highlights: inputs.highlights,
        saturation_adj: inputs.saturation_adj,
        vibrance: inputs.vibrance,
        vignette_amount: recipe.vignette_amount,
        vignette_midpoint: recipe.vignette_midpoint,
        vignette_roundness: recipe.vignette_roundness,
        vignette_feather: recipe.vignette_feather,
        highlight_desat: recipe.highlight_desat,
        use_logc: u32::from(recipe.use_logc),
        agx_look,
        zone_shadows_exposure: recipe.zone_shadows_exposure,
        zone_shadows_contrast: recipe.zone_shadows_contrast,
        zone_shadows_saturation: recipe.zone_shadows_saturation,
        zone_midtones_exposure: recipe.zone_midtones_exposure,
        zone_midtones_contrast: recipe.zone_midtones_contrast,
        zone_midtones_saturation: recipe.zone_midtones_saturation,
        zone_highlights_exposure: recipe.zone_highlights_exposure,
        zone_highlights_contrast: recipe.zone_highlights_contrast,
        zone_highlights_saturation: recipe.zone_highlights_saturation,
        curve_luma: u32::from(inputs.curves[0].is_some()),
        curve_r: u32::from(inputs.curves[1].is_some()),
        curve_g: u32::from(inputs.curves[2].is_some()),
        curve_b: u32::from(inputs.curves[3].is_some()),
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
        shadows_tint: tint4(recipe.shadows_tint),
        midtones_tint: tint4(recipe.midtones_tint),
        highlights_tint: tint4(recipe.highlights_tint),
    };

    // aux = 8 hue + 8 sat + 8 lum, then four 256-entry curve LUTs. An
    // identity curve still occupies its slot (filled with a ramp) so the
    // shader's indexing stays fixed; its flag above is what turns it off.
    let mut aux = Vec::with_capacity(24 + 4 * crate::curves::LUT_SIZE);
    for src in [&recipe.hsl_hue, &recipe.hsl_sat, &recipe.hsl_lum] {
        for i in 0..8 {
            aux.push(src.get(i).copied().unwrap_or(0.0));
        }
    }
    for curve in &inputs.curves {
        match curve {
            Some(lut) => aux.extend_from_slice(lut),
            None => aux.extend((0..crate::curves::LUT_SIZE).map(|i| i as f32 / (crate::curves::LUT_SIZE - 1) as f32)),
        }
    }

    let dev = &g.device;
    let in_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rapid-in"),
        contents: bytemuck::cast_slice(&inputs.data[..total]),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_size = (total * std::mem::size_of::<f32>()) as u64;
    let out_buf = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rapid-out"),
        size: out_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    // A zero-length storage buffer isn't bindable; one dummy float stands in
    // when no stage asked for a guidance map (has_blurred gates every read).
    let blur_src: &[f32] = if inputs.blurred.is_empty() { &[0.0] } else { inputs.blurred };
    let blur_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rapid-blurred"),
        contents: bytemuck::cast_slice(blur_src),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let params_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rapid-params"),
        contents: bytemuck::bytes_of(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let aux_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rapid-aux"),
        contents: bytemuck::cast_slice(&aux),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let staging = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rapid-staging"),
        size: out_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind = dev.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rapid-bind"),
        layout: &g.layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: in_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: out_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: blur_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: params_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: aux_buf.as_entire_binding() },
        ],
    });

    dev.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut encoder = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("rapid-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("rapid-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&g.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        // 8×8 workgroup, matching @workgroup_size in the shader.
        pass.dispatch_workgroups(
            inputs.width.div_ceil(8) as u32,
            inputs.height.div_ceil(8) as u32,
            1,
        );
    }
    encoder.copy_buffer_to_buffer(&out_buf, 0, &staging, 0, out_size);
    g.queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    dev.poll(wgpu::Maintain::Wait);
    if rx.recv().ok()?.is_err() {
        return None;
    }
    if let Some(err) = pollster::block_on(dev.pop_error_scope()) {
        eprintln!("rapid gpu: dispatch rejected, falling back to CPU: {err}");
        return None;
    }

    let out: Vec<f32> = bytemuck::cast_slice(&slice.get_mapped_range()).to_vec();
    staging.unmap();
    if out.len() < total {
        return None;
    }

    Some(ImageBuf::from_data(inputs.width as u32, inputs.height as u32, out))
}
