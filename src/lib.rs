mod engine_core;

use winit::window::{Window, WindowBuilder};
use winit::event_loop::{EventLoop};

use erupt::{vk, {EntryLoader, InstanceLoader, DeviceLoader}, {ExtendableFrom, SmallVec}, utils::{surface}};

use std::ffi::{CString};
use std::os::raw::{c_void};
use std::mem::size_of;

use engine_core::{VALIDATION_ENABLED, VALIDATION_LAYERS};
pub use engine_core::MAX_FRAMES_IN_FLIGHT;

/// Large struct for eased initialization and use of Vulkan for drawing to the screen.
/// The struct has a lot of fields to ease cleanup of the Vulkan objects (cleaned when the struct is dropped in Rust fashion),
/// as well as because many of the fields are dependant on one another, so keeping them organized together is vital to not lose track.
/// It is recommended to use the struct as a base level on top of which a user-facing application is built.
pub struct BaseApp {
    // Fields are dropped in declared order, so they must be placed in opposite order of references.
    // Changing the order will likely cause bad cleanup behaviour.
    pub sync: engine_core::SyncPrims,
    pub command_buffers: SmallVec<vk::CommandBuffer>,
    command_pool: vk::CommandPool,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub render_pass: vk::RenderPass,
    graphics_pipeline_layout: vk::PipelineLayout,
    pub graphics_pipeline: vk::Pipeline,
    _image_views: Vec<vk::ImageView>,
    pub swapchain: vk::SwapchainKHR,
    swapchain_extent: vk::Extent2D,
    pub graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    pub device: Box<DeviceLoader>,
    _surface: vk::SurfaceKHR,
    _messenger: vk::DebugUtilsMessengerEXT,
    _instance: Box<InstanceLoader>,
    _entry: Box<EntryLoader>,
}
impl Drop for BaseApp {
    fn drop(&mut self) {
        unsafe {
            for i in 0..MAX_FRAMES_IN_FLIGHT {
                self.device.destroy_semaphore(self.sync.image_available[i], None);
                self.device.destroy_semaphore(self.sync.render_finished[i], None);
                self.device.destroy_fence(self.sync.in_flight[i], None);
            }
            self.device.destroy_command_pool(self.command_pool, None);
            for buffer in &mut self.framebuffers {
                self.device.destroy_framebuffer(*buffer, None);
            }
            self.device.destroy_pipeline(self.graphics_pipeline, None);
            self.device.destroy_pipeline_layout(self.graphics_pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);
            for view in &mut self._image_views {
                self.device.destroy_image_view(*view, None);
            }
            self.device.destroy_device(None);
            if !self._messenger.is_null() {
                self._instance.destroy_debug_utils_messenger_ext(self._messenger, None)
            }
            self._instance.destroy_surface_khr(self._surface, None);
            self._instance.destroy_instance(None);
        }
        eprintln!("Engine stopped successfully");
    }
}

impl BaseApp {
    pub fn initialize_new(window: &Window, app_name: &str) -> BaseApp {
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
    
        let mut instance_extensions = surface::enumerate_required_extensions(window).unwrap();
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
        let swapchain_framebuffers = engine_core::create_framebuffers(&logical_device, render_pass, swapchain_extent, &image_views);
    
        //// Command pool and buffers
        let command_pool_info = vk::CommandPoolCreateInfoBuilder::new()
            .queue_family_index(queue_family_indices[engine_core::GRAPHICS_Q_IDX]);
        let command_pool = unsafe {logical_device.create_command_pool(&command_pool_info, None)}.expect("Could not create command pool!");
    
        let command_buffers = engine_core::allocate_command_buffers(&logical_device, command_pool, swapchain_framebuffers.len() as u32);
    
        //// Create semaphores for in-render-pass synchronization
        let sync = engine_core::create_sync_primitives(&logical_device);
    
        BaseApp {
            _entry: entry,
            _instance: instance,
            device: logical_device,
            _messenger,
            _surface: surface,
            graphics_queue,
            present_queue,
            swapchain,
            swapchain_extent,
            _image_views: image_views,
            graphics_pipeline,
            graphics_pipeline_layout,
            render_pass,
            framebuffers: swapchain_framebuffers,
            command_pool,
            command_buffers,
            sync,
        }
    }


    /// Frees the command buffers in the pool, then allocates an amount equal to the number of framebuffers.
    pub fn reallocate_command_buffers(&mut self) {
        unsafe {self.device.free_command_buffers(self.command_pool, &self.command_buffers)};

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfoBuilder::new()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(self.framebuffers.len() as u32);
        self.command_buffers = unsafe {self.device.allocate_command_buffers(&command_buffer_allocate_info)}.expect("Could not create command buffers!");
    }

    /// Iterates over each command buffer in the app, begins command buffer recording, runs the closure, then ends command buffer recording.
    /// Anything *could* be put in the closure, but the intent is Vulkan commands.
    /// # Example:
    /// ```no_run
    /// unsafe {
    ///     your_app.record_command_buffers(|app, i| {
    ///         app.device.cmd_bind_pipeline(
    ///             app.command_buffers[i],
    ///             vk::PipelineBindPoint::GRAPHICS,
    ///             app.graphics_pipeline
    ///         );
    ///     });
    /// }
    /// ```
    pub unsafe fn record_command_buffers<F>(&mut self, commands: F)
        where F: Fn(&mut BaseApp, usize)
    {
        for i in 0..self.command_buffers.len() {
            //Begin recording command buffer
            let command_buffer_begin_info = vk::CommandBufferBeginInfoBuilder::new();
            self.device.begin_command_buffer(self.command_buffers[i], &command_buffer_begin_info).expect("Could not begin command buffer recording!");

            commands(self, i);

            self.device.end_command_buffer(self.command_buffers[i]).expect("Failed recording command buffer!");
        }
    }

    /// Queues up the image at `image_index` for presentation to the window surface.
    /// Signals the given semaphore once the image has been presented.
    pub fn present_image(&self, image_index: u32, signal_semaphore: [vk::Semaphore; 1]) {
        let swapchains = [self.swapchain];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHRBuilder::new()
            .wait_semaphores(&signal_semaphore)
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        unsafe {self.device.queue_present_khr(self.present_queue, &present_info)}.expect("Presenting to queue failed!");
    }
}



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