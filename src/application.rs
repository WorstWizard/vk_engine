use crate::engine_core::{self, ManagedImage, ValidIndexBufferType, VertexInputDescriptors};
use crate::engine_core::{MAX_FRAMES_IN_FLIGHT, VALIDATION_ENABLED, VALIDATION_LAYERS};
use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{Surface, Swapchain},
    },
    vk, {Device, Entry, Instance},
};
use ash_window;
use glam::*;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::ffi::CString;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use winit::window::Window;

/** Large struct for eased initialization and use of Vulkan for drawing to the screen.
The struct has a lot of fields to ease cleanup of the Vulkan objects (cleaned when the struct is dropped in Rust fashion),
as well as because many of the fields are dependant on one another, so keeping them organized together is vital to not lose track.
It is recommended to use the struct as a base level on top of which a user-facing application is built. */
pub struct BaseApp {
    // Fields are dropped in declared order, so they must be placed in opposite order of references.
    // Changing the order will likely cause bad cleanup behaviour.
    pub sync: engine_core::SyncPrims,
    pub command_buffers: Vec<vk::CommandBuffer>,
    descriptor_pool: vk::DescriptorPool,
    pub index_buffer: ManuallyDrop<engine_core::ManagedBuffer>,
    pub vertex_buffer: ManuallyDrop<engine_core::ManagedBuffer>,
    pub uniform_buffers: ManuallyDrop<Vec<engine_core::ManagedBuffer>>,
    texture: ManuallyDrop<engine_core::ManagedImage>,
    texture_sampler: vk::Sampler,
    command_pool: vk::CommandPool,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub render_pass: vk::RenderPass,
    pub descriptor_sets: Vec<vk::DescriptorSet>,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub graphics_pipeline_layout: vk::PipelineLayout,
    pub graphics_pipeline: vk::Pipeline,
    image_views: Vec<vk::ImageView>,
    depth_image: ManuallyDrop<ManagedImage>,
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_extent: vk::Extent2D,
    swapchain_loader: Swapchain,
    pub graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    pub logical_device: Rc<Device>,
    window: Window,
    surface: vk::SurfaceKHR,
    surface_loader: Surface,
    _messenger: vk::DebugUtilsMessengerEXT,
    _debug_loader: DebugUtils,
    instance: Box<Instance>,
    _entry: Box<Entry>,
}
impl Drop for BaseApp {
    fn drop(&mut self) {
        unsafe {
            self.logical_device.device_wait_idle().unwrap(); //Wait until idle before destroying

            self.sync.destroy(&self.logical_device);

            self.logical_device
                .destroy_descriptor_pool(self.descriptor_pool, None);

            // Destroying this manually causes an error, guessing ash does it automatically on drop,
            // which it otherwise doesn't with other objects
            //self.logical_device.destroy_descriptor_set_layout(self.descriptor_set_layout.unwrap(), None);

            self.logical_device
                .destroy_sampler(self.texture_sampler, None);

            //Explicitly dropping buffers to ensure that the logical device still exists when they do
            ManuallyDrop::drop(&mut self.vertex_buffer);
            ManuallyDrop::drop(&mut self.index_buffer);
            ManuallyDrop::drop(&mut self.uniform_buffers);
            ManuallyDrop::drop(&mut self.depth_image);
            ManuallyDrop::drop(&mut self.texture);

            self.logical_device
                .destroy_command_pool(self.command_pool, None);

            self.clean_swapchain_and_dependants();

            self.logical_device.destroy_device(None);

            if VALIDATION_ENABLED {
                self._debug_loader
                    .destroy_debug_utils_messenger(self._messenger, None)
            }

            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
        eprintln!("Engine stopped successfully");
    }
}

impl BaseApp {
    pub fn new<VertexType: Sized, IndexType: ValidIndexBufferType, UBOType: Sized>(
        window: winit::window::Window,
        app_name: &str,
        shaders: &Vec<crate::shaders::Shader>,
        vertices: Vec<VertexType>,
        indices: Vec<IndexType>,
        vertex_input_descriptors: &VertexInputDescriptors,
        descriptor_set_bindings: Vec<vk::DescriptorSetLayoutBinding>,
    ) -> BaseApp {
        let entry = Box::new(unsafe { Entry::load() }.unwrap());
        if VALIDATION_ENABLED && !engine_core::check_validation_layer_support(&entry) {
            panic!("Validation layer requested but not available!");
        }

        //// Application info
        let app_name = CString::new(app_name).unwrap();
        let engine_name = CString::new("KK Engine").unwrap();

        let app_info = vk::ApplicationInfo::builder()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(&engine_name)
            .engine_version(vk::API_VERSION_1_0)
            .api_version(vk::API_VERSION_1_0);

        let mut instance_extensions =
            ash_window::enumerate_required_extensions(window.raw_display_handle())
                .unwrap()
                .to_vec();
        if VALIDATION_ENABLED {
            instance_extensions.push(DebugUtils::name().as_ptr());
        }

        //// Instance & debug messenger
        let mut messenger_info = engine_core::init_debug_messenger_info();
        let mut instance_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&instance_extensions);
        if VALIDATION_ENABLED {
            instance_info = instance_info
                .enabled_layer_names(&VALIDATION_LAYERS)
                .push_next(&mut messenger_info);
        }
        let instance = Box::new(
            unsafe { entry.create_instance(&instance_info, None) }
                .expect("Failed to create Vulkan instance!"),
        );
        let (_debug_loader, _messenger) = if VALIDATION_ENABLED {
            //Messenger attached
            let debug_loader = DebugUtils::new(&entry, &instance);
            let messenger =
                unsafe { &debug_loader.create_debug_utils_messenger(&messenger_info, None) }
                    .unwrap();
            (debug_loader, messenger)
        } else {
            (
                DebugUtils::new(&entry, &instance),
                vk::DebugUtilsMessengerEXT::default(),
            )
        };

        //// Window surface creation
        let surface_loader = Surface::new(&entry, &instance);
        let surface = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None,
            )
        }
        .unwrap();

        //// Physical device and queues
        let (physical_device, queue_family_indices) =
            engine_core::find_physical_device(&instance, &surface_loader, &surface);

        //// Logical device
        let logical_device =
            engine_core::create_logical_device(&instance, &physical_device, queue_family_indices);
        let (graphics_queue, present_queue) =
            engine_core::get_queue_handles(&logical_device, queue_family_indices);

        //// Swapchain
        let swapchain_loader = Swapchain::new(&instance, &logical_device);
        let (swapchain, image_format, swapchain_extent, swapchain_images) =
            engine_core::create_swapchain(
                &window,
                &surface_loader,
                &surface,
                &physical_device,
                &swapchain_loader,
                queue_family_indices,
            );

        //// Image views
        let image_views = engine_core::create_swapchain_image_views(
            &logical_device,
            &swapchain_images,
            image_format,
        );

        //// Push constants
        let push_constants = [1.0];

        //// Graphics pipeline
        let (graphics_pipeline, graphics_pipeline_layout, descriptor_set_layout, render_pass) =
            engine_core::create_graphics_pipeline(
                &logical_device,
                swapchain_extent,
                image_format,
                &shaders,
                vertex_input_descriptors,
                descriptor_set_bindings,
                push_constants,
            );

        //// Depth image
        // Could check for supported formats for depth, but for now just going with D32_SFLOAT
        // https://vulkan-tutorial.com/en/Depth_buffering
        let depth_image = engine_core::create_image(
            &instance,
            &physical_device,
            &logical_device,
            vk::Format::D32_SFLOAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
            (swapchain_extent.width, swapchain_extent.height),
        );

        //// Framebuffers
        let framebuffers = engine_core::create_framebuffers(
            &logical_device,
            render_pass,
            swapchain_extent,
            &image_views,
            depth_image.image_view,
        );

        //// Command pool and buffers
        let command_pool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_family_indices.graphics_queue)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = unsafe { logical_device.create_command_pool(&command_pool_info, None) }
            .expect("Could not create command pool!");

        let vertex_buffer = engine_core::create_vertex_buffer(
            &instance,
            &physical_device,
            &logical_device,
            (std::mem::size_of::<VertexType>() * vertices.len()) as u64,
        );
        {
            let vert_len = vertices.len();

            let mut staging_buffer = engine_core::create_staging_buffer(
                &instance,
                &physical_device,
                &logical_device,
                (std::mem::size_of::<VertexType>() * vert_len) as u64,
            );
            staging_buffer.map_buffer_memory();

            unsafe {
                engine_core::write_vec_to_buffer(staging_buffer.memory_ptr.unwrap(), vertices)
            };
            engine_core::copy_buffer(
                &logical_device,
                command_pool,
                graphics_queue,
                *staging_buffer,
                *vertex_buffer,
                (std::mem::size_of::<VertexType>() * vert_len) as u64,
            );
        }

        let index_buffer = engine_core::create_index_buffer(
            &instance,
            &physical_device,
            &logical_device,
            indices.len(),
        );
        {
            let indices_len = indices.len();

            let mut staging_buffer = engine_core::create_staging_buffer(
                &instance,
                &physical_device,
                &logical_device,
                (std::mem::size_of::<IndexType>() * indices_len) as u64,
            );
            staging_buffer.map_buffer_memory();

            unsafe {
                engine_core::write_vec_to_buffer(staging_buffer.memory_ptr.unwrap(), indices)
            };
            engine_core::copy_buffer(
                &logical_device,
                command_pool,
                graphics_queue,
                *staging_buffer,
                *index_buffer,
                (std::mem::size_of::<IndexType>() * indices_len) as u64,
            );
        }

        //// Uniform buffers
        let uniform_buffers = engine_core::create_uniform_buffers(
            &instance,
            &physical_device,
            &logical_device,
            std::mem::size_of::<UBOType>() as u64,
            MAX_FRAMES_IN_FLIGHT,
        );

        //// Command buffers
        let command_buffers = engine_core::allocate_command_buffers(
            &logical_device,
            command_pool,
            image_views.len() as u32,
        );

        //// Texture image
        let texture = {
            // Load image texture onto GPU
            let (img_samples, (w, h)) = crate::load_image_as_rgba_samples("texture.jpg");

            let texture_image = engine_core::create_image(
                &instance,
                &physical_device,
                &logical_device,
                vk::Format::R8G8B8A8_SRGB,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
                vk::ImageAspectFlags::COLOR,
                (w, h),
            );

            let mut tex_staging_buffer = engine_core::create_staging_buffer(
                &instance,
                &physical_device,
                &logical_device,
                vk::DeviceSize::from((w * h * 4) as u64),
            );
            tex_staging_buffer.map_buffer_memory();
            unsafe {
                engine_core::write_vec_to_buffer(
                    tex_staging_buffer.memory_ptr.unwrap(),
                    img_samples,
                )
            };

            fn transition_image_layout(
                logical_device: &Device,
                command_pool: vk::CommandPool,
                queue: vk::Queue,
                image: vk::Image,
                _format: vk::Format,
                old_layout: vk::ImageLayout,
                new_layout: vk::ImageLayout,
            ) {
                let mut barrier = vk::ImageMemoryBarrier::builder()
                    .old_layout(old_layout)
                    .new_layout(new_layout)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(
                        *vk::ImageSubresourceRange::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(1),
                    );

                let src_stage;
                let dst_stage;

                if old_layout == vk::ImageLayout::UNDEFINED
                    && new_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
                {
                    barrier = barrier
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
                    src_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
                    dst_stage = vk::PipelineStageFlags::TRANSFER;
                } else if old_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
                    && new_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
                {
                    barrier = barrier
                        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                        .dst_access_mask(vk::AccessFlags::SHADER_READ);
                    src_stage = vk::PipelineStageFlags::TRANSFER;
                    dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
                } else {
                    panic!("Image layout transition not supported!");
                }

                unsafe {
                    engine_core::immediate_commands(
                        &logical_device,
                        command_pool,
                        queue,
                        |cmd_buffer| {
                            logical_device.cmd_pipeline_barrier(
                                cmd_buffer,
                                src_stage,
                                dst_stage,
                                vk::DependencyFlags::empty(),
                                &[],
                                &[],
                                &[*barrier],
                            );
                        },
                    );
                }
            }

            fn copy_buffer_to_image(
                logical_device: &Device,
                command_pool: vk::CommandPool,
                queue: vk::Queue,
                buffer: vk::Buffer,
                image: vk::Image,
                width: u32,
                height: u32,
            ) {
                let region = vk::BufferImageCopy::builder()
                    .buffer_offset(0)
                    .buffer_row_length(0)
                    .buffer_image_height(0)
                    .image_subresource(
                        *vk::ImageSubresourceLayers::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .mip_level(0)
                            .base_array_layer(0)
                            .layer_count(1),
                    )
                    .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                    .image_extent(vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    });
                unsafe {
                    engine_core::immediate_commands(
                        logical_device,
                        command_pool,
                        queue,
                        |cmd_buffer| {
                            logical_device.cmd_copy_buffer_to_image(
                                cmd_buffer,
                                buffer,
                                image,
                                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                &[*region],
                            );
                        },
                    );
                }
            }

            transition_image_layout(
                &logical_device,
                command_pool,
                graphics_queue,
                texture_image.image,
                vk::Format::R8G8B8A8_SRGB,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );

            copy_buffer_to_image(
                &logical_device,
                command_pool,
                graphics_queue,
                tex_staging_buffer.buffer,
                texture_image.image,
                w,
                h,
            );

            transition_image_layout(
                &logical_device,
                command_pool,
                graphics_queue,
                texture_image.image,
                vk::Format::R8G8B8A8_SRGB,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );

            texture_image
        };

        let texture_sampler = {
            let max_anisotropy =
                unsafe { instance.get_physical_device_properties(physical_device) }
                    .limits
                    .max_sampler_anisotropy;
            let sampler = vk::SamplerCreateInfo::builder()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                .address_mode_w(vk::SamplerAddressMode::REPEAT)
                .anisotropy_enable(true)
                .max_anisotropy(max_anisotropy)
                .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
                .unnormalized_coordinates(false)
                .compare_enable(false)
                .compare_op(vk::CompareOp::ALWAYS)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .mip_lod_bias(0.0)
                .min_lod(0.0)
                .max_lod(0.0);
            unsafe { logical_device.create_sampler(&sampler, None) }
                .expect("Could not create texture sampler")
        };

        //// Descriptor pool
        let descriptor_pool = {
            let pool_sizes = [
                *vk::DescriptorPoolSize::builder()
                    .ty(vk::DescriptorType::UNIFORM_BUFFER)
                    .descriptor_count(MAX_FRAMES_IN_FLIGHT as u32),
                *vk::DescriptorPoolSize::builder()
                    .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .descriptor_count(MAX_FRAMES_IN_FLIGHT as u32),
            ];
            let pool_info = vk::DescriptorPoolCreateInfo::builder()
                .pool_sizes(&pool_sizes)
                .max_sets(MAX_FRAMES_IN_FLIGHT as u32);
            unsafe { logical_device.create_descriptor_pool(&pool_info, None) }
                .expect("Failed to create descriptor pool")
        };

        //// Descriptor sets
        let descriptor_sets = {
            let layouts = vec![descriptor_set_layout; MAX_FRAMES_IN_FLIGHT];
            let alloc_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(descriptor_pool)
                .set_layouts(layouts.as_slice());
            unsafe { logical_device.allocate_descriptor_sets(&alloc_info) }
                .expect("Failed to allocate descriptor sets")
        };
        let descriptor_writes = {
            let mut v = Vec::with_capacity(descriptor_sets.len());
            for (i, set) in descriptor_sets.iter().enumerate() {
                let descriptor_buffer_info = [*vk::DescriptorBufferInfo::builder()
                    .buffer(*uniform_buffers[i])
                    .offset(0)
                    .range(std::mem::size_of::<UBOType>() as u64)];
                let descriptor_image_info = [*vk::DescriptorImageInfo::builder()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(texture.image_view)
                    .sampler(texture_sampler)];
                v.push(
                    *vk::WriteDescriptorSet::builder()
                        .dst_set(*set)
                        .dst_binding(0)
                        .dst_array_element(0)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(&descriptor_buffer_info),
                );
                v.push(
                    *vk::WriteDescriptorSet::builder()
                        .dst_set(*set)
                        .dst_binding(1)
                        .dst_array_element(0)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&descriptor_image_info),
                );
            }
            v
        };
        unsafe { logical_device.update_descriptor_sets(&descriptor_writes, &[]) }

        //// Create semaphores for in-render-pass synchronization
        let sync = engine_core::create_sync_primitives(&logical_device);

        BaseApp {
            _entry: entry,
            instance,
            logical_device,
            _debug_loader,
            _messenger,
            window,
            surface_loader,
            surface,
            graphics_queue,
            present_queue,
            swapchain_loader,
            swapchain,
            swapchain_extent,
            image_views,
            depth_image: ManuallyDrop::new(depth_image),
            graphics_pipeline,
            graphics_pipeline_layout,
            descriptor_set_layout,
            descriptor_sets,
            render_pass,
            framebuffers,
            command_pool,
            vertex_buffer: ManuallyDrop::new(vertex_buffer),
            index_buffer: ManuallyDrop::new(index_buffer),
            uniform_buffers: ManuallyDrop::new(uniform_buffers),
            texture: ManuallyDrop::new(texture),
            texture_sampler,
            descriptor_pool,
            command_buffers,
            sync,
        }
    }

    /** Acquire index of image from the swapchain, signal semaphore once finished.
    If the error is of type `ERROR_OUT_OF_DATE_KHR`, the swapchain needs to be recreated before rendering can resume.
    May also return error `SUBOPTIMAL_KHR`, in which case the swapchain *should* be recreated.
    Returns a boolean that also indicates suboptimality, [`ash`] provides it so we just propagate it
    # Example:
    ```ignore
    let (image_index, _) = match app.acquire_next_image(frame_idx) {
        Ok((i, _)) => i,
        Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
            app.recreate_swapchain();
            return
        },
        _ => panic!("Could not acquire image from swapchain!")
    };
    ``` */
    pub fn acquire_next_image(
        &mut self,
        framebuffer_index: usize,
    ) -> Result<(u32, bool), vk::Result> {
        unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.sync.image_available[framebuffer_index],
                vk::Fence::null(),
            )
        }
    }

    /// Blocks host execution, waiting for the fence at `self.sync.in_flight[fence_index]` to be signaled. No timeout.
    pub fn wait_for_in_flight_fence(&self, fence_index: usize) {
        let wait_fences = [self.sync.in_flight[fence_index]];
        unsafe {
            self.logical_device
                .wait_for_fences(&wait_fences, true, u64::MAX)
        }
        .unwrap();
    }

    /// Resets fence at `self.sync.in_flight[fence_index]`. No timeout.
    pub fn reset_in_flight_fence(&self, fence_index: usize) {
        let wait_fences = [self.sync.in_flight[fence_index]];
        unsafe { self.logical_device.reset_fences(&wait_fences) }.unwrap();
    }

    /** Begins command buffer recording, runs the closure, then ends command buffer recording.
    Anything *could* be put in the closure, but the intent is Vulkan commands.
    # Example:
    ```ignore
    unsafe {
        base_app.record_command_buffer(buf_index, |app| {
            app.device.cmd_bind_pipeline(
                app.command_buffers[buf_index],
                vk::PipelineBindPoint::GRAPHICS,
                app.graphics_pipeline
            );
        });
    }
    ``` */
    pub unsafe fn record_command_buffer<F>(&mut self, buffer_index: usize, commands: F)
    where
        F: Fn(&mut BaseApp),
    {
        //Begin recording command buffer
        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder();
        self.logical_device
            .begin_command_buffer(
                self.command_buffers[buffer_index],
                &command_buffer_begin_info,
            )
            .expect("Could not begin command buffer recording!");

        commands(self);

        self.logical_device
            .end_command_buffer(self.command_buffers[buffer_index])
            .expect("Failed recording command buffer!");
    }

    /*
    /// Frees the command buffers in the pool, then allocates an amount equal to the number of framebuffers.
    pub fn reallocate_command_buffers(&mut self) {
        unsafe {self.logical_device.free_command_buffers(self.command_pool, &self.command_buffers)};

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(self.framebuffers.len() as u32);
        self.command_buffers = unsafe {self.logical_device.allocate_command_buffers(&command_buffer_allocate_info)}.expect("Could not create command buffers!");
    }
    */

    /** Submits the command buffer at `buffer_index` to the graphics queue, waiting for a swapchain image:`self.sync.image_available[buffer_index]`.
    Waits for the `COLOR_ATTACHMENT_OUTPUT` stage, then executes commands. Once the image has been drawn, `self.sync.render_finished[buffer_index]` is signaled,
    and the `self.sync.in_flight[buffer_index]` fence is signaled. */
    pub fn submit_drawing_command_buffer(&self, buffer_index: usize) {
        let wait_sems = [self.sync.image_available[buffer_index]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_sems = [self.sync.render_finished[buffer_index]];
        let cmd_buffers = [self.command_buffers[buffer_index]];
        let submits = [*vk::SubmitInfo::builder()
            .wait_semaphores(&wait_sems)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&cmd_buffers)
            .signal_semaphores(&signal_sems)];
        unsafe {
            self.logical_device
                .queue_submit(
                    self.graphics_queue,
                    &submits,
                    self.sync.in_flight[buffer_index],
                )
                .expect("Queue submission failed!");
        }
    }

    /** Queues up the image at `image_index` for presentation to the window surface.
    Signals the given semaphore once the image has been presented.
    If the error is of type `ERROR_OUT_OF_DATE_KHR`, the swapchain needs to be recreated before rendering can resume.
    Recommended to recreate the swapchain also if the error is type `SUBOPTIMAL_KHR`
    # Example:
    ```ignore
    match vulkan_app.present_image(image_index, signal_sems) {
    Ok(()) => (),
    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
        vulkan_app.recreate_swapchain();
        return
        },
        _ => panic!("Could not present image!")
    };
    ``` */
    pub fn present_image(
        &self,
        image_index: u32,
        wait_semaphore: vk::Semaphore,
    ) -> Result<bool, vk::Result> {
        let swapchain_arr = [self.swapchain];
        let image_index_arr = [image_index];
        let wait_semaphore_arr = [wait_semaphore];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&wait_semaphore_arr)
            .swapchains(&swapchain_arr)
            .image_indices(&image_index_arr);
        unsafe {
            self.swapchain_loader
                .queue_present(self.present_queue, &present_info)
        }
    }

    /** Recreates the swapchain and the dependants of the swapchain.
    Necessary if some condition changes that invalidates the swapchain, most commonly a window resize.
    Excessive resizing of the window will cause rare Vulkan validation errors due to a data race in [`engine_core::create_swapchain`],
    where the extent of the window may change after it has been queried to set the swapchain extent, but before the swapchain is created.
    This error is non-fatal and largely unpreventable without a lot of runtime checks in that function, so for now it is ignored */
    pub fn recreate_swapchain(
        &mut self,
        shaders: &Vec<crate::shaders::Shader>,
        vertex_input_descriptors: &VertexInputDescriptors,
        descriptor_set_bindings: Vec<vk::DescriptorSetLayoutBinding>,
    ) {
        unsafe {
            self.logical_device.device_wait_idle().unwrap();
            self.clean_swapchain_and_dependants();
        }

        let (physical_device, queue_family_indices) =
            engine_core::find_physical_device(&self.instance, &self.surface_loader, &self.surface);
        let (swapchain, image_format, swapchain_extent, swapchain_images) =
            engine_core::create_swapchain(
                &self.window,
                &self.surface_loader,
                &self.surface,
                &physical_device,
                &self.swapchain_loader,
                queue_family_indices,
            );
        let image_views = engine_core::create_swapchain_image_views(
            &self.logical_device,
            &swapchain_images,
            image_format,
        );
        let (graphics_pipeline, graphics_pipeline_layout, descriptor_set_layout, render_pass) =
            engine_core::create_graphics_pipeline(
                &self.logical_device,
                swapchain_extent,
                image_format,
                shaders,
                vertex_input_descriptors,
                descriptor_set_bindings,
                [0.0],
            );
        let depth_image = engine_core::create_image(
            &self.instance,
            &physical_device,
            &self.logical_device,
            vk::Format::D32_SFLOAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
            (swapchain_extent.width, swapchain_extent.height),
        );
        let framebuffers = engine_core::create_framebuffers(
            &self.logical_device,
            render_pass,
            swapchain_extent,
            &image_views,
            depth_image.image_view,
        );

        unsafe { ManuallyDrop::drop(&mut self.depth_image) };
        self.depth_image = ManuallyDrop::new(depth_image);

        self.swapchain = swapchain;
        self.swapchain_extent = swapchain_extent;
        self.image_views = image_views;
        self.render_pass = render_pass;
        self.graphics_pipeline = graphics_pipeline;
        self.graphics_pipeline_layout = graphics_pipeline_layout;
        self.descriptor_set_layout = descriptor_set_layout;
        self.framebuffers = framebuffers;
    }

    unsafe fn clean_swapchain_and_dependants(&mut self) {
        for buffer in self.framebuffers.drain(..) {
            self.logical_device.destroy_framebuffer(buffer, None);
        }
        self.logical_device
            .destroy_pipeline(self.graphics_pipeline, None);
        self.logical_device
            .destroy_pipeline_layout(self.graphics_pipeline_layout, None);
        self.logical_device
            .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        self.logical_device
            .destroy_render_pass(self.render_pass, None);
        for view in self.image_views.drain(..) {
            self.logical_device.destroy_image_view(view, None);
        }
        self.swapchain_loader
            .destroy_swapchain(self.swapchain, None);
    }
}
