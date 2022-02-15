#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] //Required to prevent console window from appearing on Windows

use vk_engine::*;
use std::time;
use erupt::vk;
use winit::event::{Event, WindowEvent, VirtualKeyCode};
use winit::event_loop::{ControlFlow};

const MAX_FRAMES_IN_FLIGHT: usize = 2;

fn main() {
    let (window, event_loop) = init_window();
    let mut vulkan_app = init_vulkan(&window);
    let mut current_frame = 0;
    let mut timer = time::Instant::now();
    let speed = 0.1;
    let mut push_constants = [0.0];
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


                let wait_fences = [vulkan_app.in_flight_fences[current_frame]];
                unsafe {vulkan_app.device.wait_for_fences(&wait_fences, true, u64::MAX)}.unwrap();

                // Acquire index of image from the swapchain, signal semaphore once finished
                let image_index = unsafe {
                    vulkan_app.device.acquire_next_image_khr(
                        vulkan_app.swapchain,
                        u64::MAX,
                        vulkan_app.image_available_sems[current_frame],
                        vk::Fence::null()
                    ).unwrap()
                };

                // Is the requested image already in-flight? Then wait for it to finish
                if !vulkan_app.images_in_flight[image_index as usize].is_null() {
                    let wait_fences = [vulkan_app.images_in_flight[image_index as usize]];
                    unsafe {vulkan_app.device.wait_for_fences(&wait_fences, true, u64::MAX)}.unwrap();
                }
                // The image is now being used by this frame
                vulkan_app.images_in_flight[image_index as usize] = vulkan_app.in_flight_fences[current_frame];

                //Reallocate to get the new push constants in, lazy mans method
                if zooming {
                    let time_delta = timer.elapsed();
                    push_constants[0] = (push_constants[0] + time_delta.as_secs_f32()*speed) % 2.0;//(2.0*3.1415926535);

                    let amount = vulkan_app.command_buffers.len();
                    unsafe {vulkan_app.device.free_command_buffers(vulkan_app.command_pool, &vulkan_app.command_buffers)};
                    vulkan_app.command_buffers = allocate_and_record_command_buffers(
                        amount as u32,
                        vulkan_app.command_pool,
                        &vulkan_app.device,
                        vulkan_app.swapchain_extent,
                        &vulkan_app.framebuffers,
                        vulkan_app.renderpass,
                        vulkan_app.graphics_pipeline,
                        vulkan_app.graphics_pipeline_layout,
                        &push_constants
                    );    
                }

                let wait_sems = [vulkan_app.image_available_sems[current_frame]];
                let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
                let signal_sems = [vulkan_app.render_finished_sems[current_frame]];
                let cmd_buffers = [vulkan_app.command_buffers[image_index as usize]];
                let submits = [vk::SubmitInfoBuilder::new()
                    .wait_semaphores(&wait_sems)
                    .wait_dst_stage_mask(&wait_stages)
                    .command_buffers(&cmd_buffers)
                    .signal_semaphores(&signal_sems)];
                unsafe {
                    vulkan_app.device.reset_fences(&wait_fences).unwrap();
                    vulkan_app.device.queue_submit(vulkan_app.graphics_queue, &submits, vulkan_app.in_flight_fences[current_frame]).expect("Queue submission failed!");
                }

                // Present rendered image to the swap chain such that it will show up on screen
                vulkan_app.present_image(image_index, signal_sems);
                timer = time::Instant::now(); //Reset timer after frame is presented

                current_frame = current_frame % MAX_FRAMES_IN_FLIGHT;

                //window.request_redraw() //Call if state changed and a redraw is necessary
            },
            Event::RedrawRequested(_) => { //Conditionally redraw (OS might request this too)

            },
            Event::LoopDestroyed => {
                println!("Exiting event loop, should drop application");
                unsafe {
                    vulkan_app.device.device_wait_idle().unwrap(); //App referred to in closure, it is dropped once the scope closes
                }
            }
            _ => ()
        }
    });
}