//! Slim library for more easily making simple graphical Vulkan applications.
//! Actively developed, everything may change and break.
//! There is yet no entirely consistent rule for which functions are safe/unsafe.

//  Author: Kristian Knudsen

pub mod engine_core;
pub mod engine_app;

use winit::window::{Window, WindowBuilder};
use winit::event_loop::{EventLoop};

use std::os::raw::{c_void};
use std::mem::size_of;

use erupt::vk;

pub use engine_app::BaseApp;

pub fn init_window(app_name: &str, width: u32, height: u32) -> (Window, EventLoop<()>) {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size( winit::dpi::PhysicalSize::new(width, height))
        .with_title(app_name)
        .with_resizable(false)
        .build(&event_loop).expect("Window build failed!");
    (window, event_loop)
}

/// For use inside [`BaseApp::record_command_buffers`]. Will cover most common use cases for drawing:
/// 1. Sets the render area to the full swapchain extent and sets the (first) clear color to black
/// 2. Begins a render pass and binds the graphics pipeline to the graphics stage
/// 3. Runs `commands` closure
/// 4. Ends render pass
pub unsafe fn drawing_commands<F>(app: &mut BaseApp, index: usize, commands: F, push_constants: &[f32; 1])
    where F: FnOnce(&mut BaseApp, usize)
{
    //Start render pass
    let render_area = vk::Rect2DBuilder::new()
        .offset(vk::Offset2D{x: 0, y: 0})
        .extent(app.swapchain_extent);
    let mut clear_color = [vk::ClearValue::default()]; clear_color[0].color.float32 = [0.0, 0.0, 0.0, 1.0];
    let renderpass_begin_info = vk::RenderPassBeginInfoBuilder::new()
        .render_pass(app.render_pass)
        .framebuffer(app.framebuffers[index])
        .render_area(*render_area)
        .clear_values(&clear_color);
    app.device.cmd_begin_render_pass(app.command_buffers[index], &renderpass_begin_info, vk::SubpassContents::INLINE);
    app.device.cmd_bind_pipeline(app.command_buffers[index], vk::PipelineBindPoint::GRAPHICS, app.graphics_pipeline);
    app.device.cmd_push_constants(app.command_buffers[index], app.graphics_pipeline_layout, vk::ShaderStageFlags::VERTEX, 0, (push_constants.len()*size_of::<f32>()) as u32, push_constants.as_ptr() as *const c_void);

    commands(app, index);

    //End the render pass
    app.device.cmd_end_render_pass(app.command_buffers[index]);    
}