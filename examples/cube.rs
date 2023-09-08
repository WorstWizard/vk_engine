#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] //Required to prevent console window from appearing on Windows

use ash::vk;
use glam::{vec3, vec2, Mat4, Quat, Vec3, Vec2};
use std::mem::size_of;
use std::time;
use vk_engine::engine_core::write_struct_to_buffer;
use vk_engine::{init_window, uniform_buffer_descriptor_set_layout_bindings, BaseApp};
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::ControlFlow;

const APP_TITLE: &str = "KK Engine Test App";

#[repr(C)]
struct Vertex {
    pos: Vec3,
    tex: Vec2
}

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
        Vertex{pos: vec3(-0.5, -0.5, -0.5), tex: vec2(0.0, 0.0)},
        Vertex{pos: vec3(0.5, -0.5, -0.5), tex: vec2(1.0, 0.0)},
        Vertex{pos: vec3(-0.5, 0.5, -0.5), tex: vec2(0.0, 1.0)},
        Vertex{pos: vec3(0.5, 0.5, -0.5), tex: vec2(1.0, 1.0)},
        Vertex{pos: vec3(-0.5, -0.5, 0.5), tex: vec2(1.0, 0.0)},
        Vertex{pos: vec3(0.5, -0.5, 0.5), tex: vec2(0.0, 0.0)},
        Vertex{pos: vec3(-0.5, 0.5, 0.5), tex: vec2(1.0, 1.0)},
        Vertex{pos: vec3(0.5, 0.5, 0.5), tex: vec2(0.0, 1.0)},
    ];
    let indices: Vec<u16> = vec![
        0, 1, 2, //front
        1, 3, 2, 5, 4, 6, //back
        5, 6, 7, 4, 0, 6, //left
        0, 2, 6, 1, 5, 3, //right
        5, 7, 3, 4, 5, 0, //top
        5, 1, 0, 2, 3, 6, //bottom
        3, 7, 6,
    ];

    let num_indices = indices.len() as u32;

    let vertex_input_descriptors = {
        let binding = vec![*vk::VertexInputBindingDescription::builder()
            .binding(0)
            .input_rate(vk::VertexInputRate::VERTEX)
            .stride(size_of::<Vertex>() as u32)];
        let attribute = vec![
            *vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(0),
            *vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(12) // Careful! Should use a function for this, difficult in rust without using a crate
        ];

        vk_engine::VertexInputDescriptors {
            bindings: binding,
            attributes: attribute,
        }
    };

    // Uniform buffer object
    let ubo_vec: Vec<vk_engine::MVP> = vec![vk_engine::MVP {
        model: Mat4::from_translation(vec3(0.0, 0.0, 5.0)),
        view: Mat4::look_at_rh(
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, -1.0, 0.0),
        ),
        projection: Mat4::perspective_infinite_rh(f32::to_radians(90.0), 1.0, 0.01),
    }];
    let ubo_bindings = uniform_buffer_descriptor_set_layout_bindings(1);

    let mut vulkan_app = BaseApp::new(
        window,
        APP_TITLE,
        &shaders_loaded,
        verts,
        indices,
        &vertex_input_descriptors,
        Some(ubo_vec),
        Some(ubo_bindings.clone()),
    );

    //Tracks which frame the CPU is currently writing commands for
    //*Not* a framecounter, this value is mod MAX_FRAMES_IN_FLIGHT
    let mut current_frame = 0;

    //For the animation
    let mut timer = time::Instant::now();
    let speed = 0.3;
    let mut spinning = true;
    let mut theta = 0.0;

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
                // On some platforms (occurs on Windows 10 as of writing), the swapchain is not marked as suboptimal/out-of-date when
                // the window is resized, so here it is polled explicitly via winit to ensure the swapchain remains correctly sized
                WindowEvent::Resized(_) => {
                    vulkan_app.recreate_swapchain(
                        &shaders_loaded,
                        &vertex_input_descriptors,
                        Some(ubo_bindings.clone()),
                    );
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                // Main body

                // Wait for this frame's command buffer to finish execution (image presented)
                vulkan_app.wait_for_in_flight_fence(current_frame);

                // Acquire index of image from the swapchain, signal semaphore once finished
                let (image_index, _) = match vulkan_app.acquire_next_image(current_frame) {
                    Ok(i) => i,
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        //Swapchain is outdated, recreate it before continuing
                        vulkan_app.recreate_swapchain(
                            &shaders_loaded,
                            &vertex_input_descriptors,
                            Some(ubo_bindings.clone()),
                        );
                        return; //Exits current event loop iteration
                    }
                    _ => panic!("Could not acquire image from swapchain!"),
                };

                // Reset fence. This is done now, since if the swapchain is outdated, it causes an early return to the event loop
                vulkan_app.reset_in_flight_fence(current_frame);

                // Change time constant if spinning is enabled
                if spinning {
                    let time_delta = timer.elapsed();
                    theta = (theta + time_delta.as_secs_f32() * speed) % (2.0 * 3.14159265);
                }

                let eye = Vec3::new(0.0, -1.0, 0.0);
                let model_center = Vec3::new(0.0, 0.0, 2.0);
                let up_direction = Vec3::new(0.0, -1.0, 0.0);
                let aspect_ratio = vulkan_app.swapchain_extent.width as f32
                    / vulkan_app.swapchain_extent.height as f32;

                let model =
                    Mat4::from_rotation_translation(Quat::from_rotation_y(theta), model_center);
                let view = Mat4::look_at_lh(eye, model_center, up_direction);
                let projection =
                    Mat4::perspective_infinite_lh(f32::to_radians(90.0), aspect_ratio, 0.01);

                let mut correction_mat = Mat4::IDENTITY;
                correction_mat.y_axis = glam::Vec4::new(0.0, -1.0, 0.0, 0.0);

                let ubo = vk_engine::MVP {
                    model,
                    view: correction_mat.mul_mat4(&view),
                    projection,
                };
                // Copy data to uniform buffer
                unsafe {
                    write_struct_to_buffer(
                        vulkan_app.uniform_buffers[current_frame]
                            .memory_ptr
                            .expect("Uniform buffer memory has not been mapped!"),
                        &ubo as *const vk_engine::MVP,
                    )
                };

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
                            &[0.0],
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
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        //Swapchain might be outdated again
                        vulkan_app.recreate_swapchain(
                            &shaders_loaded,
                            &vertex_input_descriptors,
                            Some(ubo_bindings.clone()),
                        );
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
