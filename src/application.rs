use ash::{vk,
    {Entry, Instance, Device},
    extensions::{
        khr::{Surface, Swapchain},
        ext::DebugUtils}};
use raw_window_handle::{HasRawWindowHandle, HasRawDisplayHandle};
use ash_window;
use winit::window::Window;
use std::ffi::CString;
use std::rc::Rc;
use std::mem::ManuallyDrop;
use crate::engine_core::{VALIDATION_ENABLED, VALIDATION_LAYERS, MAX_FRAMES_IN_FLIGHT};
use crate::engine_core;

/** Large struct for eased initialization and use of Vulkan for drawing to the screen.
The struct has a lot of fields to ease cleanup of the Vulkan objects (cleaned when the struct is dropped in Rust fashion),
as well as because many of the fields are dependant on one another, so keeping them organized together is vital to not lose track.
It is recommended to use the struct as a base level on top of which a user-facing application is built. */
pub struct BaseApp {
    // Fields are dropped in declared order, so they must be placed in opposite order of references.
    // Changing the order will likely cause bad cleanup behaviour.
    pub sync: engine_core::SyncPrims,
    pub command_buffers: Vec<vk::CommandBuffer>,
    pub index_buffer: ManuallyDrop<engine_core::ManagedBuffer>,
    pub vertex_buffer: ManuallyDrop<engine_core::ManagedBuffer>,
    command_pool: vk::CommandPool,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub render_pass: vk::RenderPass,
    pub graphics_pipeline_layout: vk::PipelineLayout,
    pub graphics_pipeline: vk::Pipeline,
    image_views: Vec<vk::ImageView>,
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

            for i in 0..MAX_FRAMES_IN_FLIGHT {
                self.logical_device.destroy_semaphore(self.sync.image_available[i], None);
                self.logical_device.destroy_semaphore(self.sync.render_finished[i], None);
                self.logical_device.destroy_fence(self.sync.in_flight[i], None);
            }

            //Explicitly dropping buffers to ensure that the logical device still exists when they do
            ManuallyDrop::drop(&mut self.vertex_buffer);
            ManuallyDrop::drop(&mut self.index_buffer);

            self.logical_device.destroy_command_pool(self.command_pool, None);

            self.clean_swapchain_and_dependants();

            self.logical_device.destroy_device(None);

            if VALIDATION_ENABLED {
                self._debug_loader.destroy_debug_utils_messenger(self._messenger, None)
            }

            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
        eprintln!("Engine stopped successfully");
    }
}

impl BaseApp {
    pub fn new(window: winit::window::Window, app_name: &str, shaders: (crate::shaders::Shader, crate::shaders::Shader)) -> BaseApp {
        let entry = Box::new(unsafe{ Entry::load() }.unwrap());
    
        if VALIDATION_ENABLED && !engine_core::check_validation_layer_support(&entry) {
            panic!("Validation layer requested but not available!");
        }
    
        //// Application info
        let app_name = CString::new(app_name).unwrap();
        let engine_name = CString::new("KK Engine").unwrap();
    
        let app_info = vk::ApplicationInfo::builder()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0,1,0,0))
            .engine_name(&engine_name)
            .engine_version(vk::API_VERSION_1_0)
            .api_version(vk::API_VERSION_1_0);
    
        let mut instance_extensions = ash_window::enumerate_required_extensions(window.raw_display_handle()).unwrap().to_vec();
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
        let instance = Box::new(unsafe {entry.create_instance(&instance_info, None)}.expect("Failed to create Vulkan instance!"));
        let (_debug_loader, _messenger) = if VALIDATION_ENABLED { //Messenger attached
            let debug_loader = DebugUtils::new(&entry, &instance);
            let messenger = unsafe {&debug_loader.create_debug_utils_messenger(&messenger_info, None)}.unwrap();
            (debug_loader, messenger)
        } else {
            (DebugUtils::new(&entry, &instance), vk::DebugUtilsMessengerEXT::default())
        };
    
        //// Window surface creation
        let surface_loader = Surface::new(&entry, &instance);
        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, window.raw_display_handle(), window.raw_window_handle(), None)
        }.unwrap();
    
        //// Physical device and queues
        let (physical_device, queue_family_indices) = engine_core::find_physical_device(&instance, &surface_loader, &surface);
    
        //// Logical device
        let logical_device = engine_core::create_logical_device(&instance, &physical_device, queue_family_indices);
        let (graphics_queue, present_queue) = engine_core::get_queue_handles(&logical_device, queue_family_indices);
    
        //// Swapchain
        let swapchain_loader = Swapchain::new(&instance, &logical_device);
        let (swapchain, image_format, swapchain_extent, swapchain_images) =
        engine_core::create_swapchain(&window, &surface_loader, &surface, &physical_device, &swapchain_loader, queue_family_indices);
    
        //// Image views
        let image_views = engine_core::create_image_views(&logical_device, &swapchain_images, image_format);
    
        //// Push constants
        let push_constants = [1.0];
    
        //// Graphics pipeline
        let (graphics_pipeline, graphics_pipeline_layout, render_pass) = engine_core::create_graphics_pipeline(&logical_device, swapchain_extent, image_format, shaders, push_constants);
    
        //// Framebuffers
        let framebuffers = engine_core::create_framebuffers(&logical_device, render_pass, swapchain_extent, &image_views);
    
        //// Command pool and buffers
        let command_pool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_family_indices[engine_core::GRAPHICS_Q_IDX])
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = unsafe {logical_device.create_command_pool(&command_pool_info, None)}.expect("Could not create command pool!");

        let verts = vec![
            engine_core::Vert(-1.0, -1.0),
            engine_core::Vert( 1.0, -1.0),
            engine_core::Vert(-1.0,  1.0),
            engine_core::Vert( 1.0,  1.0),
        ];

        let indices: Vec<u16> = vec![0,1,2,1,3,2];

        let vertex_buffer = engine_core::create_vertex_buffer(&instance, &physical_device, &logical_device, verts.len());
        {
            let mut staging_buffer = engine_core::create_staging_buffer(&instance, &physical_device, &logical_device, (std::mem::size_of::<engine_core::Vert>() * 4) as u64);
            let staging_pointer = staging_buffer.map_buffer_memory();

            unsafe { engine_core::write_vec_to_buffer(staging_pointer, verts) };
            engine_core::copy_buffer(&logical_device, command_pool, graphics_queue, *staging_buffer, *vertex_buffer, (std::mem::size_of::<engine_core::Vert>() * 4) as u64);
        }

        let index_buffer = engine_core::create_index_buffer(&instance, &physical_device, &logical_device, 6); //6 indices necessary to specify rect
        {   
            let mut staging_buffer = engine_core::create_staging_buffer(&instance, &physical_device, &logical_device, (std::mem::size_of::<u16>() * 6) as u64);
            let staging_pointer = staging_buffer.map_buffer_memory();

            unsafe { engine_core::write_vec_to_buffer(staging_pointer, indices) };
            engine_core::copy_buffer(&logical_device, command_pool, graphics_queue, *staging_buffer, *index_buffer, (std::mem::size_of::<u16>() * 6) as u64);
        }

        let command_buffers = engine_core::allocate_command_buffers(&logical_device, command_pool, image_views.len() as u32);
    
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
            surface: surface,
            graphics_queue,
            present_queue,
            swapchain_loader,
            swapchain,
            swapchain_extent,
            image_views,
            graphics_pipeline,
            graphics_pipeline_layout,
            render_pass,
            framebuffers,
            command_pool,
            vertex_buffer: ManuallyDrop::new(vertex_buffer),
            index_buffer: ManuallyDrop::new(index_buffer),
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
    pub fn acquire_next_image(&mut self, framebuffer_index: usize) -> Result<(u32, bool), vk::Result> {
        unsafe {
            self.swapchain_loader.acquire_next_image(self.swapchain, u64::MAX, self.sync.image_available[framebuffer_index], vk::Fence::null())
        }
    }

    /// Blocks host execution, waiting for the fence at `self.sync.in_flight[fence_index]` to be signaled. No timeout.
    pub fn wait_for_in_flight_fence(&self, fence_index: usize) {
        let wait_fences = [self.sync.in_flight[fence_index]];
        unsafe {self.logical_device.wait_for_fences(&wait_fences, true, u64::MAX)}.unwrap();
    }

    /// Resets fence at `self.sync.in_flight[fence_index]`. No timeout.
    pub fn reset_in_flight_fence(&self, fence_index: usize) {
        let wait_fences = [self.sync.in_flight[fence_index]];
        unsafe {self.logical_device.reset_fences(&wait_fences)}.unwrap();
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
        where F: Fn(&mut BaseApp)
    {
        //Begin recording command buffer
        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder();
        self.logical_device.begin_command_buffer(self.command_buffers[buffer_index], &command_buffer_begin_info).expect("Could not begin command buffer recording!");

        commands(self);

        self.logical_device.end_command_buffer(self.command_buffers[buffer_index]).expect("Failed recording command buffer!");        
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
        let cmd_buffers = [self.command_buffers[buffer_index as usize]];
        let submits = [vk::SubmitInfo::builder()
            .wait_semaphores(&wait_sems)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&cmd_buffers)
            .signal_semaphores(&signal_sems)
            .build()];
        unsafe {
            self.logical_device.queue_submit(self.graphics_queue, &submits, self.sync.in_flight[buffer_index]).expect("Queue submission failed!");
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
    pub fn present_image(&self, image_index: u32, signal_semaphore: vk::Semaphore) -> Result<bool, vk::Result> {
        let swapchain_arr = [self.swapchain];
        let image_index_arr = [image_index];
        let signal_semaphore_arr = [signal_semaphore];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&signal_semaphore_arr)
            .swapchains(&swapchain_arr)
            .image_indices(&image_index_arr);
        unsafe {
            self.swapchain_loader.queue_present(self.present_queue, &present_info)
        }
    }

    /** Recreates the swapchain and the dependants of the swapchain.
    Necessary if some condition changes that invalidates the swapchain, most commonly a window resize.
    Excessive resizing of the window will cause rare Vulkan validation errors due to a data race in [`engine_core::create_swapchain`],
    where the extent of the window may change after it has been queried to set the swapchain extent, but before the swapchain is created.
    This error is non-fatal and largely unpreventable without a lot of runtime checks in that function, so for now it is ignored */
    pub fn recreate_swapchain(&mut self, shaders: (crate::shaders::Shader, crate::shaders::Shader)) {
        unsafe {
            self.logical_device.device_wait_idle().unwrap();
            self.clean_swapchain_and_dependants();
        }

        let (physical_device, queue_family_indices) = engine_core::find_physical_device(&self.instance, &self.surface_loader, &self.surface);
        let (swapchain, image_format, swapchain_extent, swapchain_images) =
        engine_core::create_swapchain(&self.window, &self.surface_loader, &self.surface, &physical_device, &self.swapchain_loader, queue_family_indices);
        let image_views = engine_core::create_image_views(&self.logical_device, &swapchain_images, image_format);
        let (graphics_pipeline, graphics_pipeline_layout, render_pass) = engine_core::create_graphics_pipeline(&self.logical_device, swapchain_extent, image_format, shaders, [0.0]);
        let framebuffers = engine_core::create_framebuffers(&self.logical_device, render_pass, swapchain_extent, &image_views);

        self.swapchain = swapchain;
        self.swapchain_extent = swapchain_extent;
        self.image_views = image_views;
        self.render_pass = render_pass;
        self.graphics_pipeline = graphics_pipeline;
        self.graphics_pipeline_layout = graphics_pipeline_layout;
        self.framebuffers = framebuffers;
    }

    unsafe fn clean_swapchain_and_dependants(&mut self) {
        for buffer in self.framebuffers.drain(..) {
            self.logical_device.destroy_framebuffer(buffer, None);
        }
        self.logical_device.destroy_pipeline(self.graphics_pipeline, None);
        self.logical_device.destroy_pipeline_layout(self.graphics_pipeline_layout, None);
        self.logical_device.destroy_render_pass(self.render_pass, None);
        for view in self.image_views.drain(..) {
            self.logical_device.destroy_image_view(view, None);
        }
        self.swapchain_loader.destroy_swapchain(self.swapchain, None);
    }
}