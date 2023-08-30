#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] //Required to prevent console window from appearing on Windows

use ash::vk;
use glam::{vec3, Vec3, Mat4};
use std::mem::size_of;
use std::time;
use vk_engine::{init_window, BaseApp, uniform_buffer_descriptor_set_layout_bindings};
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::ControlFlow;

const APP_TITLE: &str = "KK Engine Test App";

fn main() {
    let (window, event_loop) = init_window(APP_TITLE, 1000, 1000);
    let shaders_loaded = vec![
        vk_engine::shaders::compile_shader(
            "examples/shaders/cube.vert",
            None,
            vk_engine::shaders::ShaderType::Vertex,
        )
        .unwrap(),
        vk_engine::shaders::compile_shader(
            "examples/shaders/cube.frag",
            None,
            vk_engine::shaders::ShaderType::Fragment,
        )
        .unwrap(),
    ];

    // Vertices of a cube
    let verts = vec![
        vec3(-0.5, -0.5, -0.5),
        vec3(0.5, -0.5, -0.5),
        vec3(-0.5, 0.5, -0.5),
        vec3(0.5, 0.5, -0.5),
        vec3(-0.5, -0.5, 0.5),
        vec3(0.5, -0.5, 0.5),
        vec3(-0.5, 0.5, 0.5),
        vec3(0.5, 0.5, 0.5),
    ];
    let indices: Vec<u16> = vec![
        0, 2, 1,
        2, 3, 1,
        // 1, 7, 5,
        // 1, 3, 7,
        // 4, 5, 6,
        // 5, 7, 6,
    ];
    let num_indices = indices.len() as u32;

    let vertex_input_descriptors = {
        let binding = vec![*vk::VertexInputBindingDescription::builder()
            .binding(0)
            .input_rate(vk::VertexInputRate::VERTEX)
            .stride(size_of::<Vec3>() as u32)];
        let attribute = vec![*vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(0)];

        vk_engine::VertexInputDescriptors {
            bindings: binding,
            attributes: attribute,
        }
    };

    // Uniform buffer object
    let ubo = vec![vk_engine::MVP {
        model: Mat4::from_translation(vec3(0.0, 0.0, 5.0)),
        view: Mat4::IDENTITY,
        projection: Mat4::perspective_infinite_rh(f32::to_radians(90.0), 1.0, 0.01)
    }];
    let ubo_bindings = uniform_buffer_descriptor_set_layout_bindings(ubo.clone());

    let mut vulkan_app = BaseApp::new(
        window,
        APP_TITLE,
        &shaders_loaded,
        verts,
        indices,
        &vertex_input_descriptors,
        Some(ubo),
        Some(ubo_bindings.clone())
    );

    //Tracks which frame the CPU is currently writing commands for
    //*Not* a framecounter, this value is mod MAX_FRAMES_IN_FLIGHT
    let mut current_frame = 0;

    //For the animation
    let mut push_constants = [0.0];
    let mut timer = time::Instant::now();
    let speed = 0.1;
    let mut spinning = true;

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
                            spinning = !spinning;
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
                        vulkan_app.recreate_swapchain(&shaders_loaded, &vertex_input_descriptors, Some(ubo_bindings.clone()));
                        return; //Exits current event loop iteration
                    }
                    _ => panic!("Could not acquire image from swapchain!"),
                };

                // Reset fence. This is done now, since if the swapchain is outdated, it causes an early return to the event loop
                vulkan_app.reset_in_flight_fence(current_frame);

                // Change time constant if spinning is enabled
                if spinning {
                    let time_delta = timer.elapsed();
                    push_constants[0] =
                        (push_constants[0] + time_delta.as_secs_f32() * speed) % 2.0;
                }

                // Copy data to uniform buffer
                //vulkan_app.uniform_buffers[0]

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
                                    num_indices,
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
                        vulkan_app.recreate_swapchain(&shaders_loaded, &vertex_input_descriptors, Some(ubo_bindings.clone()));
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
