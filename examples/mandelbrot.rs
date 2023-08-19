#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] //Required to prevent console window from appearing on Windows

use ash::vk;
use glam::vec2;
use std::mem::size_of;
use std::rc::Rc;
use std::time;
use vk_engine::{init_window, BaseApp};
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::ControlFlow;

const APP_TITLE: &str = "KK Engine Test App";

fn main() {
    let (window, event_loop) = init_window(APP_TITLE, 1000, 1000);
    let shaders_loaded = vec![
        vk_engine::shaders::load_shader(
            "examples/shaders_compiled/mandelbrot.vert.spv",
            vk_engine::shaders::ShaderType::Vertex,
        )
        .unwrap(),
        vk_engine::shaders::load_shader(
            "examples/shaders_compiled/mandelbrot.frag.spv",
            vk_engine::shaders::ShaderType::Fragment,
        )
        .unwrap(),
    ];

    // Vertices
    let verts = vec![
        vec2(-1.0, -1.0),
        vec2(1.0, -1.0),
        vec2(-1.0, 1.0),
        vec2(1.0, 1.0),
    ];
    let indices: Vec<u16> = vec![0, 1, 2, 1, 3, 2];
    
    let vertex_input_descriptors = {

        let binding = vec![*vk::VertexInputBindingDescription::builder()
            .binding(0)
            .input_rate(vk::VertexInputRate::VERTEX)
            .stride(size_of::<glam::Vec2>() as u32)];
        let attribute = vec![*vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0)];
        
        vk_engine::VertexInputDescriptors{
            bindings: Rc::new(binding),
            attributes: Rc::new(attribute),
        }
    };
    let mut vulkan_app = BaseApp::new(window, APP_TITLE, &shaders_loaded, verts, indices, &vertex_input_descriptors);

    //Tracks which frame the CPU is currently writing commands for
    //*Not* a framecounter, this value is mod MAX_FRAMES_IN_FLIGHT
    let mut current_frame = 0;

    //For the animation
    let mut push_constants = [0.0];
    let mut timer = time::Instant::now();
    let speed = 0.1;
    let mut zooming = true;

    //The event loop hijacks the main thread, so once it closes the entire program exits.
    //All cleanup operations should be handled either before the main loop, inside the mainloop,
    //or in the drop function of any data moved into the closure
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::KeyboardInput { input, .. } => match input.virtual_keycode {
                    Some(VirtualKeyCode::Space) => {
                        if input.state == winit::event::ElementState::Pressed {
                            zooming = !zooming;
                        }
                    }
                    Some(VirtualKeyCode::Escape) => {
                        *control_flow = ControlFlow::Exit;
                    }
                    _ => (),
                },
                _ => (),
            },
            Event::MainEventsCleared => {
                // Main body

                // Wait for this frame's command buffer to finish execution (image presented)
                vulkan_app.wait_for_in_flight_fence(current_frame);

                // Acquire index of image from the swapchain, signal semaphore once finished
                let (image_index, _) = match vulkan_app.acquire_next_image(current_frame) {
                    Ok(i) => i,
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                        //Swapchain is outdated, recreate it before continuing
                        vulkan_app.recreate_swapchain(&shaders_loaded, &vertex_input_descriptors);
                        return; //Exits current event loop iteration
                    },
                    _ => panic!("Could not acquire image from swapchain!"),
                };

                // Reset fence. This is done now, since if the swapchain is outdated, it causes an early return to the event loop
                vulkan_app.reset_in_flight_fence(current_frame);

                // Change time constant if zooming is enabled
                if zooming {
                    let time_delta = timer.elapsed();
                    push_constants[0] =
                        (push_constants[0] + time_delta.as_secs_f32() * speed) % 2.0;
                }

                // Record drawing commands into command buffer for current frame
                unsafe {
                    vulkan_app.record_command_buffer(current_frame, |app| {
                        vk_engine::drawing_commands(
                            app,
                            current_frame,
                            image_index,
                            |app| {
                                app.logical_device.cmd_draw_indexed(
                                    app.command_buffers[current_frame],
                                    6,
                                    1,
                                    0,
                                    0,
                                    0,
                                );
                            },
                            &push_constants,
                        );
                    })
                };

                // Submit commands to render image
                vulkan_app.submit_drawing_command_buffer(current_frame);

                // Present rendered image to the swap chain such that it will show up on screen
                match vulkan_app
                    .present_image(image_index, vulkan_app.sync.render_finished[current_frame])
                {
                    Ok(_) => (),
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                        //Swapchain might be outdated again
                        vulkan_app.recreate_swapchain(&shaders_loaded, &vertex_input_descriptors);
                        return;
                    }
                    _ => panic!("Could not present image!"),
                };

                timer = time::Instant::now(); //Reset timer after frame is presented
                current_frame = (current_frame + 1) % vk_engine::engine_core::MAX_FRAMES_IN_FLIGHT;
                //Advance to next frame
            }
            Event::RedrawRequested(_) => { //Conditionally redraw (OS might request this too)
            }
            Event::LoopDestroyed => {
                println!("Exiting event loop, should drop application");
            }
            _ => (),
        }
    });
}
