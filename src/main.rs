#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] //Required to prevent console window from appearing on Windows

use vk_engine::{BaseApp, init_window};
use std::time;
use erupt::vk;
use winit::event::{Event, WindowEvent, VirtualKeyCode};
use winit::event_loop::{ControlFlow};

const APP_TITLE: &str = "KK Engine Test App";

fn main() {
    let (window, event_loop) = init_window(APP_TITLE, 1000, 1000);
    let shaders_loaded = (
        vk_engine::shaders::load_shader("shaders_compiled/mandelbrot.vert.spv", vk_engine::shaders::ShaderType::Vertex).unwrap(),
        vk_engine::shaders::load_shader("shaders_compiled/mandelbrot.frag.spv", vk_engine::shaders::ShaderType::Fragment).unwrap(),
    );
    let mut vulkan_app = BaseApp::new(window, APP_TITLE, shaders_loaded.clone());

    let mut push_constants = [0.0];
    unsafe {vulkan_app.record_command_buffers(|app, i| {
        vk_engine::drawing_commands(app, i, |app, i| {
            app.logical_device.cmd_draw(app.command_buffers[i], 4, 1, 0, 0);
        }, &push_constants)
    })};

    let mut current_frame = 0;
    let mut timer = time::Instant::now();
    let speed = 0.1;
    let mut zooming = true;

    //The event loop hijacks the main thread, so once it closes the entire program exits.
    //All cleanup operations should be handled either before the main loop, inside the mainloop,
    //or in the drop function of any data moved into the closure
    event_loop.run(move |event,_,control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent{event, ..} => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                },
                WindowEvent::KeyboardInput{input,..} => {
                    match input.virtual_keycode {
                        Some(VirtualKeyCode::Space) => {
                            if input.state == winit::event::ElementState::Pressed {
                                zooming = !zooming;
                            }
                        },
                        Some(VirtualKeyCode::Escape) => {
                            *control_flow = ControlFlow::Exit;
                        }
                        _ => (),
                    }
                },
                _ => (),
            },
            Event::MainEventsCleared => { //Main body
                //If drawing continously, put rendering code here directly
                
                //Wait for this frame's command buffer to finish execution (image presented)
                let wait_fences = [vulkan_app.sync.in_flight[current_frame]];
                unsafe {vulkan_app.logical_device.wait_for_fences(&wait_fences, true, u64::MAX)}.unwrap();
                // Acquire index of image from the swapchain, signal semaphore once finished
                let image_index = match vulkan_app.acquire_next_image(current_frame) {
                    Ok(i) => i,
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        vulkan_app.recreate_swapchain(shaders_loaded.clone());
                        return
                    },
                    _ => panic!("Could not acquire image from swapchain!")
                };
                unsafe {vulkan_app.logical_device.reset_fences(&wait_fences)}.unwrap(); //Reset the corresponding fence


                //Reallocate to get the new push constants in, lazy mans method
                if zooming {
                    let time_delta = timer.elapsed();
                    push_constants[0] = (push_constants[0] + time_delta.as_secs_f32()*speed) % 2.0;//(2.0*3.1415926535);

                    vulkan_app.reallocate_command_buffers();
                    unsafe {vulkan_app.record_command_buffers(|app, i| {
                        vk_engine::drawing_commands(app, i, |app, i| {
                            app.logical_device.cmd_draw_indexed(app.command_buffers[i], 6, 1, 0, 0, 0);
                            //app.logical_device.cmd_draw(app.command_buffers[i], 4, 1, 0, 0);
                        }, &push_constants);
                    })};
                }

                // Submit rendered image
                let wait_sems = [vulkan_app.sync.image_available[current_frame]];
                let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
                let signal_sems = [vulkan_app.sync.render_finished[current_frame]];
                let cmd_buffers = [vulkan_app.command_buffers[image_index as usize]];
                let submits = [vk::SubmitInfoBuilder::new()
                    .wait_semaphores(&wait_sems)
                    .wait_dst_stage_mask(&wait_stages)
                    .command_buffers(&cmd_buffers)
                    .signal_semaphores(&signal_sems)];
                unsafe {
                    vulkan_app.logical_device.queue_submit(vulkan_app.graphics_queue, &submits, vulkan_app.sync.in_flight[current_frame]).expect("Queue submission failed!");
                }

                // Present rendered image to the swap chain such that it will show up on screen
                match vulkan_app.present_image(image_index, signal_sems) {
                    Ok(()) => (),
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                        vulkan_app.recreate_swapchain(shaders_loaded.clone());
                        return
                    },
                    _ => panic!("Could not present image!")
                };
                timer = time::Instant::now(); //Reset timer after frame is presented

                current_frame = current_frame % vk_engine::engine_core::MAX_FRAMES_IN_FLIGHT;
            },
            Event::RedrawRequested(_) => { //Conditionally redraw (OS might request this too)
            },
            Event::LoopDestroyed => {
                println!("Exiting event loop, should drop application");
            }
            _ => ()
        }
    });
}