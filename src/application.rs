use erupt::{vk, {EntryLoader, InstanceLoader, DeviceLoader}, {ExtendableFrom, SmallVec}, utils::{surface}};
use winit::window::Window;
use std::ffi::{CString};
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
    pub command_buffers: SmallVec<vk::CommandBuffer>,
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
    pub graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    pub logical_device: Rc<DeviceLoader>,
    window: Window,
    surface: vk::SurfaceKHR,
    _messenger: vk::DebugUtilsMessengerEXT,
    instance: Box<InstanceLoader>,
    _entry: Box<EntryLoader>,
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
            if !self._messenger.is_null() {
                self.instance.destroy_debug_utils_messenger_ext(self._messenger, None)
            }

            self.instance.destroy_surface_khr(self.surface, None);
            self.instance.destroy_instance(None);
        }
        eprintln!("Engine stopped successfully");
    }
}

impl BaseApp {
    pub fn initialize_new(window: winit::window::Window, app_name: &str) -> BaseApp {
        let entry = Box::new(EntryLoader::new().unwrap());
    
        if VALIDATION_ENABLED && !engine_core::check_validation_layer_support(&entry) {
            panic!("Validation layer requested but not available!");
        }
    
        //// Application info
        let app_name = CString::new(app_name).unwrap();
        let engine_name = CString::new("KK Engine").unwrap();
    
        let app_info = vk::ApplicationInfoBuilder::new()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0,1,0,0))
            .engine_name(&engine_name)
            .engine_version(vk::API_VERSION_1_0)
            .api_version(vk::API_VERSION_1_0);
    
        let mut instance_extensions = surface::enumerate_required_extensions(&window).unwrap();
        if VALIDATION_ENABLED {
            instance_extensions.push(vk::EXT_DEBUG_UTILS_EXTENSION_NAME);
        }
    
        //// Instance & debug messenger
        let mut messenger_info = engine_core::init_debug_messenger_info();
        let mut instance_info = vk::InstanceCreateInfoBuilder::new()
            .application_info(&app_info)
            .enabled_extension_names(&instance_extensions);
        if VALIDATION_ENABLED {
            instance_info = instance_info
                .enabled_layer_names(&VALIDATION_LAYERS)
                .extend_from(&mut messenger_info);
        }
        let instance = Box::new(unsafe {InstanceLoader::new(&entry, &instance_info)}.expect("Failed to create Vulkan instance!"));
        let _messenger = if VALIDATION_ENABLED { //Messenger attached
            unsafe {instance.create_debug_utils_messenger_ext(&messenger_info, None)}.unwrap()
        } else {
            vk::DebugUtilsMessengerEXT::default()
        };
    
        //// Window surface creation
        let surface = unsafe { surface::create_surface(&instance, &window, None) }.unwrap();
    
        //// Physical device and queues
        let (physical_device, queue_family_indices) = engine_core::find_physical_device(&instance, &surface);
    
        //// Logical device
        let logical_device = engine_core::create_logical_device(&instance, &physical_device, queue_family_indices);
        let (graphics_queue, present_queue) = engine_core::get_queue_handles(&logical_device, queue_family_indices);
    
        //// Swapchain
        let (swapchain, image_format, swapchain_extent, swapchain_images) =
        engine_core::create_swapchain(&instance, &window, &surface, &physical_device, &logical_device, queue_family_indices);
    
        //// Image views
        let image_views = engine_core::create_image_views(&logical_device, &swapchain_images, image_format);
    
        //// Push constants
        let push_constants = [1.0];
    
        //// Graphics pipeline
        let (graphics_pipeline, graphics_pipeline_layout, render_pass) = engine_core::create_graphics_pipeline(&logical_device, swapchain_extent, image_format, push_constants);
    
        //// Framebuffers
        let framebuffers = engine_core::create_framebuffers(&logical_device, render_pass, swapchain_extent, &image_views);
    
        //// Command pool and buffers
        let command_pool_info = vk::CommandPoolCreateInfoBuilder::new()
            .queue_family_index(queue_family_indices[engine_core::GRAPHICS_Q_IDX]);
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
            _messenger,
            window,
            surface: surface,
            graphics_queue,
            present_queue,
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

    /** Iterates over each command buffer in the app, begins command buffer recording, runs the closure, then ends command buffer recording.
    Anything *could* be put in the closure, but the intent is Vulkan commands.
    # Example:
    ```no_run
    unsafe {
        base_app.record_command_buffers(|app, i| {
            app.device.cmd_bind_pipeline(
                app.command_buffers[i],
                vk::PipelineBindPoint::GRAPHICS,
                app.graphics_pipeline
            );
        });
    }
    ``` */
    pub unsafe fn record_command_buffers<F>(&mut self, commands: F)
        where F: Fn(&mut BaseApp, usize)
    {
        for i in 0..self.command_buffers.len() {
            //Begin recording command buffer
            let command_buffer_begin_info = vk::CommandBufferBeginInfoBuilder::new();
            self.logical_device.begin_command_buffer(self.command_buffers[i], &command_buffer_begin_info).expect("Could not begin command buffer recording!");

            commands(self, i);

            self.logical_device.end_command_buffer(self.command_buffers[i]).expect("Failed recording command buffer!");
        }
    }

    /// Frees the command buffers in the pool, then allocates an amount equal to the number of framebuffers.
    pub fn reallocate_command_buffers(&mut self) {
        unsafe {self.logical_device.free_command_buffers(self.command_pool, &self.command_buffers)};

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfoBuilder::new()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(self.framebuffers.len() as u32);
        self.command_buffers = unsafe {self.logical_device.allocate_command_buffers(&command_buffer_allocate_info)}.expect("Could not create command buffers!");
    }

    /** Acquire index of image from the swapchain, signal semaphore once finished.
    If the error is of type `ERROR_OUT_OF_DATE_KHR`, the swapchain needs to be recreated before rendering can resume.
    # Example:
    ```
    let image_index = match app.acquire_next_image(frame_idx) {
        Ok(i) => i,
        Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
            app.recreate_swapchain();
            return
        },
        _ => panic!("Could not acquire image from swapchain!")
    };
    ``` */
    pub fn acquire_next_image(&mut self, framebuffer_index: usize) -> Result<u32, vk::Result> {
        unsafe {
            self.logical_device.acquire_next_image_khr(self.swapchain, u64::MAX, self.sync.image_available[framebuffer_index], vk::Fence::null()).result()
        }
    }

    /** Queues up the image at `image_index` for presentation to the window surface.
    Signals the given semaphore once the image has been presented.
    If the error is of type `ERROR_OUT_OF_DATE_KHR`, the swapchain needs to be recreated before rendering can resume.
    Recommended to recreate the swapchain also if the error is type `SUBOPTIMAL_KHR`
    # Example:
    ```
    match vulkan_app.present_image(image_index, signal_sems) {
    Ok(()) => (),
    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
        vulkan_app.recreate_swapchain();
        return
        },
        _ => panic!("Could not present image!")
    };
    ``` */
    pub fn present_image(&self, image_index: u32, signal_semaphore: [vk::Semaphore; 1]) -> Result<(), vk::Result> {
        let swapchains = [self.swapchain];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHRBuilder::new()
            .wait_semaphores(&signal_semaphore)
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        unsafe {
            self.logical_device.queue_present_khr(self.present_queue, &present_info).result()
        }
    }

    /** Recreates the swapchain and the dependants of the swapchain.
    Necessary if some condition changes that invalidates the swapchain, most commonly a window resize.
    Extensive resizing of the window will cause rare Vulkan validation errors due to a data race in [`engine_core::create_swapchain`],
    where the extent of the window may change after it has been queried to set the swapchain extent, but before the swapchain is created.
    This error is non-fatal and largely unpreventable without a lot of runtime checks in that function, so for now is ignored */
    pub fn recreate_swapchain(&mut self) {
        unsafe {
            self.logical_device.device_wait_idle().unwrap();
            self.clean_swapchain_and_dependants();
        }

        let (physical_device, queue_family_indices) = engine_core::find_physical_device(&self.instance, &self.surface);
        let (swapchain, image_format, swapchain_extent, swapchain_images) =
        engine_core::create_swapchain(&self.instance, &self.window, &self.surface, &physical_device, &self.logical_device, queue_family_indices);
        let image_views = engine_core::create_image_views(&self.logical_device, &swapchain_images, image_format);
        let (graphics_pipeline, graphics_pipeline_layout, render_pass) = engine_core::create_graphics_pipeline(&self.logical_device, swapchain_extent, image_format, [0.0]);
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
        self.logical_device.destroy_swapchain_khr(self.swapchain, None);
    }
}