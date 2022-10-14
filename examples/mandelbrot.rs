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
        vk_engine::shaders::load_shader("examples/shaders_compiled/mandelbrot.vert.spv", vk_engine::shaders::ShaderType::Vertex).unwrap(),
        vk_engine::shaders::load_shader("examples/shaders_compiled/mandelbrot.frag.spv", vk_engine::shaders::ShaderType::Fragment).unwrap(),
    );
    let mut vulkan_app = BaseApp::new(window, APP_TITLE, shaders_loaded.clone());

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

                // Change time constant if zooming is enabled
                if zooming {
                    let time_delta = timer.elapsed();
                    push_constants[0] = (push_constants[0] + time_delta.as_secs_f32()*speed) % 2.0;//(2.0*3.1415926535);
                }

                // Record drawing commands into command buffer for current frame
                unsafe {
                    vulkan_app.record_command_buffer(current_frame, |app| {
                    vk_engine::drawing_commands(app, current_frame, image_index, |app| {
                        app.logical_device.cmd_draw_indexed(app.command_buffers[current_frame], 6, 1, 0, 0, 0);
                    }, &push_constants);
                })};

                // Submit commands to render image
                vulkan_app.submit_drawing_command_buffer(current_frame);

                // Present rendered image to the swap chain such that it will show up on screen
                match vulkan_app.present_image(image_index, vulkan_app.sync.render_finished[current_frame]) {
                    Ok(_) => (),
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                        vulkan_app.recreate_swapchain(shaders_loaded.clone());
                        return
                    },
                    _ => panic!("Could not present image!")
                };

                timer = time::Instant::now(); //Reset timer after frame is presented
                current_frame = (current_frame + 1) % vk_engine::engine_core::MAX_FRAMES_IN_FLIGHT; //Advance to next frame
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