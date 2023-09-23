//  Author: Kristian Knudsen
/*!
Slim library for more easily making simple graphical Vulkan applications.
Actively developed, everything may change and break.
There is yet no entirely consistent rule for which functions are safe/unsafe.

### Crate features
* **shader_compilation** -
    Provides functions for runtime compilation of shaders using [shaderc](https://crates.io/crates/shaderc)
*/

use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

use ash::vk;

/// Core functionality used to build the [`BaseApp`],
/// can be used as shortcuts for custom Vulkan applications where [`BaseApp`] cannot be extended to cover needs.
pub mod engine_core;

#[doc(hidden)]
pub mod application;

/// Managing shaders
pub mod shaders;

#[doc(inline)]
pub use application::BaseApp;
pub use engine_core::VertexInputDescriptors;

/// Quick initialization of a window
pub fn init_window(app_name: &str, width: u32, height: u32) -> (Window, EventLoop<()>) {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
        .with_title(app_name)
        .with_resizable(true)
        .build(&event_loop)
        .expect("Window build failed!");
    (window, event_loop)
}

/**
For use inside [`BaseApp::record_command_buffer`]. Will cover most common use cases for drawing:
1. Sets the render area to the full swapchain extent and sets the (first) clear color to black
2. Begins a render pass and binds the graphics pipeline to the graphics stage
3. Runs `commands` closure
4. Ends render pass
# Safety
Behaviour is undefined if the arguments are invalid.
*/
pub unsafe fn drawing_commands<F>(
    app: &mut BaseApp,
    buffer_index: usize,
    swapchain_image_index: u32,
    commands: F,
    push_constants: &[f32; 1],
    index_type: vk::IndexType
) where
    F: FnOnce(&mut BaseApp),
{
    //Start render pass
    let render_area = vk::Rect2D::builder()
        .offset(vk::Offset2D { x: 0, y: 0 })
        .extent(app.swapchain_extent);
    let mut clear_values = [vk::ClearValue::default(); 2];
    clear_values[0].color.float32 = [0.0, 0.0, 0.0, 1.0];
    clear_values[1].depth_stencil = vk::ClearDepthStencilValue {
        depth: 1.0,
        stencil: 0,
    };
    let renderpass_begin_info = vk::RenderPassBeginInfo::builder()
        .render_pass(app.render_pass)
        .framebuffer(app.framebuffers[swapchain_image_index as usize])
        .render_area(*render_area)
        .clear_values(&clear_values);
    app.logical_device.cmd_begin_render_pass(
        app.command_buffers[buffer_index],
        &renderpass_begin_info,
        vk::SubpassContents::INLINE,
    );
    app.logical_device.cmd_bind_pipeline(
        app.command_buffers[buffer_index],
        vk::PipelineBindPoint::GRAPHICS,
        app.graphics_pipeline,
    );
    app.logical_device.cmd_push_constants(
        app.command_buffers[buffer_index],
        app.graphics_pipeline_layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        push_constants
            .iter()
            .flat_map(|float| (*float).to_ne_bytes())
            .collect::<Vec<u8>>()
            .as_slice(),
    );
    let vertex_buffers = [app.vertex_buffer.buffer];
    let offsets = [0];
    app.logical_device.cmd_bind_vertex_buffers(
        app.command_buffers[buffer_index],
        0,
        &vertex_buffers,
        &offsets,
    );
    app.logical_device.cmd_bind_index_buffer(
        app.command_buffers[buffer_index],
        app.index_buffer.buffer,
        0,
        index_type,
    );
    app.logical_device.cmd_bind_descriptor_sets(
        app.command_buffers[buffer_index],
        vk::PipelineBindPoint::GRAPHICS,
        app.graphics_pipeline_layout,
        0,
        &[app.descriptor_sets[buffer_index]],
        &[],
    );

    commands(app);

    //End the render pass
    app.logical_device
        .cmd_end_render_pass(app.command_buffers[buffer_index]);
}

// Struct for for MVP matrices, to be used in uniform buffers
#[repr(C)]
#[derive(Clone)]
pub struct MVP {
    pub model: glam::Mat4,
    pub view: glam::Mat4,
    pub projection: glam::Mat4,
}

/**
Default descriptor set layout bindings:
Binding 0: Uniform buffer, used in vertex stage
Binding 1: Image sampler, used in fragment stage
*/
pub fn default_descriptor_set_layout_bindings() -> Vec<vk::DescriptorSetLayoutBinding> {
    vec![
        *vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX),
        *vk::DescriptorSetLayoutBinding::builder()
            .binding(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT),
    ]
}

pub fn load_image_as_rgba_samples(img_path: &str) -> (Vec<u8>, (u32, u32)) {
    let img = image::io::Reader::open(img_path)
        .expect(&format!("Could not open '{}'", img_path))
        .decode()
        .expect(&format!("Could not decode '{}'", img_path));
    let img_size = (img.width(), img.height());
    let pixels = img.into_rgba8().into_flat_samples().samples;

    (pixels, img_size)
}
