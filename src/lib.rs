mod engine_core;

use winit::window::{Window, WindowBuilder};
use winit::event_loop::{EventLoop};

use erupt::{vk, {EntryLoader, InstanceLoader, DeviceLoader}, {ExtendableFrom, SmallVec}, utils::{surface}};

use std::ffi::{CString};
use std::os::raw::{c_void};
use std::mem::size_of;

use engine_core::{VALIDATION_ENABLED, VALIDATION_LAYERS, MAX_FRAMES_IN_FLIGHT};

const HEIGHT: u32 = 800;
const WIDTH: u32 = 800;
const APP_TITLE: &str = "VK Engine by KK";


pub fn init_window() -> (Window, EventLoop<()>) {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size( winit::dpi::PhysicalSize::new(WIDTH, HEIGHT))
        .with_title(APP_TITLE)
        .with_resizable(false)
        .build(&event_loop).expect("Window build failed!");
    (window, event_loop)
}

pub struct VulkanApp { //Members dropped in declared order. So they must be placed in opposite order of references
    pub sync: engine_core::SyncPrims,
    pub command_buffers: SmallVec<vk::CommandBuffer>,
    pub command_pool: vk::CommandPool,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub render_pass: vk::RenderPass,
    pub graphics_pipeline_layout: vk::PipelineLayout,
    pub graphics_pipeline: vk::Pipeline,
    pub image_views: Vec<vk::ImageView>,
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_extent: vk::Extent2D,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub device: Box<DeviceLoader>,
    pub surface: vk::SurfaceKHR,
    _messenger: vk::DebugUtilsMessengerEXT,
    pub instance: Box<InstanceLoader>,
    _entry: Box<EntryLoader>,
}
impl Drop for VulkanApp {
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
            for view in &mut self.image_views {
                self.device.destroy_image_view(*view, None);
            }
            self.device.destroy_device(None);
            if !self._messenger.is_null() {
                self.instance.destroy_debug_utils_messenger_ext(self._messenger, None)
            }
            self.instance.destroy_surface_khr(self.surface, None);
            self.instance.destroy_instance(None);
        }
        eprintln!("Engine stopped successfully");
    }
}
pub fn init_vulkan(window: &Window) -> VulkanApp {
    let entry = Box::new(EntryLoader::new().unwrap());

    if VALIDATION_ENABLED && !engine_core::check_validation_layer_support(&entry) {
        panic!("Validation layer requested but not available!");
    }

    //// Application info
    let app_name = CString::new(APP_TITLE).unwrap();
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
    let mut messenger_info = init_debug_messenger_info();
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

    let command_buffers = allocate_and_record_command_buffers(
        swapchain_images.len() as u32,
        command_pool,
        &logical_device,
        swapchain_extent,
        &swapchain_framebuffers,
        render_pass,
        graphics_pipeline,
        graphics_pipeline_layout,
        &push_constants
    );

    //// Create semaphores for in-render-pass synchronization
    let sync = engine_core::create_sync_primitives(&logical_device, swapchain_images);

    VulkanApp {
        _entry: entry,
        instance,
        device: logical_device,
        _messenger,
        surface,
        graphics_queue,
        present_queue,
        swapchain,
        swapchain_extent,
        image_views,
        graphics_pipeline,
        graphics_pipeline_layout,
        render_pass,
        framebuffers: swapchain_framebuffers,
        command_pool,
        command_buffers,
        sync,
    }
}

fn init_debug_messenger_info() -> vk::DebugUtilsMessengerCreateInfoEXTBuilder<'static> {
    let messenger_info = vk::DebugUtilsMessengerCreateInfoEXTBuilder::new()
    .message_severity(
        //vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE_EXT |
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING_EXT |
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR_EXT
    )
    .message_type(
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL_EXT |
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION_EXT |
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE_EXT
    )
    .pfn_user_callback(Some(engine_core::debug_callback));
    messenger_info
}

pub fn allocate_and_record_command_buffers(
    amount: u32,
    command_pool: vk::CommandPool,
    logical_device: &DeviceLoader,
    swapchain_extent: vk::Extent2D,
    swapchain_framebuffers: &Vec<vk::Framebuffer>,
    renderpass: vk::RenderPass,
    graphics_pipeline: vk::Pipeline,
    graphics_pipeline_layout: vk::PipelineLayout,
    push_constants: &[f32; 1]
) -> SmallVec<vk::CommandBuffer> {

    let command_buffer_allocate_info = vk::CommandBufferAllocateInfoBuilder::new()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(amount);
    let command_buffers = unsafe {logical_device.allocate_command_buffers(&command_buffer_allocate_info)}.expect("Could not create command buffers!");

    for i in 0..command_buffers.len() {
        //Begin recording command buffer
        let command_buffer_begin_info = vk::CommandBufferBeginInfoBuilder::new();
        unsafe {logical_device.begin_command_buffer(command_buffers[i], &command_buffer_begin_info)}.expect("Could not begin command buffer recording!");

        //Start render pass
        let render_area = vk::Rect2DBuilder::new()
            .offset(vk::Offset2D{x: 0, y: 0})
            .extent(swapchain_extent);
        let mut clear_color = [vk::ClearValue::default()]; clear_color[0].color.float32 = [0.0, 0.0, 0.0, 1.0];
        let renderpass_begin_info = vk::RenderPassBeginInfoBuilder::new()
            .render_pass(renderpass)
            .framebuffer(swapchain_framebuffers[i])
            .render_area(*render_area)
            .clear_values(&clear_color);
        unsafe {logical_device.cmd_begin_render_pass(command_buffers[i], &renderpass_begin_info, vk::SubpassContents::INLINE)};

        //Drawing commands
        unsafe {
            logical_device.cmd_bind_pipeline(command_buffers[i], vk::PipelineBindPoint::GRAPHICS, graphics_pipeline);
            logical_device.cmd_push_constants(command_buffers[i], graphics_pipeline_layout, vk::ShaderStageFlags::VERTEX,0, (push_constants.len()*size_of::<f32>()) as u32, push_constants.as_ptr() as *const c_void);
            logical_device.cmd_draw(command_buffers[i], 4, 1, 0, 0);
            //In order: vertexCount, instanceCount, firstVertex, firstInstance
        }

        //End the render pass and end recording
        unsafe {
            logical_device.cmd_end_render_pass(command_buffers[i]);    
            logical_device.end_command_buffer(command_buffers[i]).expect("Failed recording command buffer!");
        }
    }
    return command_buffers;
}

impl VulkanApp {
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