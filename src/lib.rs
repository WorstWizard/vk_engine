mod engine_core;
mod engine_shaders;

use winit::window::{Window, WindowBuilder};
use winit::event_loop::{EventLoop};

use erupt::{vk, {EntryLoader, InstanceLoader, DeviceLoader}, {ExtendableFrom, SmallVec}, utils::{surface}};

use std::ffi::{CString};
use std::os::raw::{c_void};
use std::mem::size_of;

use engine_core::{VALIDATION_ENABLED, VALIDATION_LAYERS};

const HEIGHT: u32 = 800;
const WIDTH: u32 = 800;
const APP_TITLE: &str = "VK Engine by KK";


// Shaders
const VERT_SHADER: &[u8] = include_bytes!("man_vert.spv");
const FRAG_SHADER: &[u8] = include_bytes!("man_frag.spv");

const MAX_FRAMES_IN_FLIGHT: usize = 2;

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
    pub image_available_sems: SmallVec<vk::Semaphore>,
    pub render_finished_sems: SmallVec<vk::Semaphore>,
    pub in_flight_fences: SmallVec<vk::Fence>,
    pub images_in_flight: SmallVec<vk::Fence>,
    pub command_buffers: SmallVec<vk::CommandBuffer>,
    pub command_pool: vk::CommandPool,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub renderpass: vk::RenderPass,
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
                self.device.destroy_semaphore(self.image_available_sems[i], None);
                self.device.destroy_semaphore(self.render_finished_sems[i], None);
                //images_in_flight actually references the same structs, so in_flight_fences being destroyed cleans it up too
                self.device.destroy_fence(self.in_flight_fences[i], None);
            }
            self.device.destroy_command_pool(self.command_pool, None);
            for buffer in &mut self.framebuffers {
                self.device.destroy_framebuffer(*buffer, None);
            }
            self.device.destroy_pipeline(self.graphics_pipeline, None);
            self.device.destroy_pipeline_layout(self.graphics_pipeline_layout, None);
            self.device.destroy_render_pass(self.renderpass, None);
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
        eprintln!("Engine stopped succesfully");
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
    let (graphics_pipeline, graphics_pipeline_layout, renderpass) = {
        // Render pass
        let color_attachments = [vk::AttachmentDescriptionBuilder::new()
            .format(image_format)
            .samples(vk::SampleCountFlagBits::_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
        // Subpass
        let dependencies = [vk::SubpassDependencyBuilder::new()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];
        let color_attachment_refs = [vk::AttachmentReferenceBuilder::new()
            .attachment(0) //First attachment in array -> color_attachment
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let subpasses = [vk::SubpassDescriptionBuilder::new()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_refs)];
        
        let renderpass_info = vk::RenderPassCreateInfoBuilder::new()
            .attachments(&color_attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);
        let renderpass = unsafe {logical_device.create_render_pass(&renderpass_info, None)}.expect("Failed to create renderpass!");


        let entry_point = CString::new("main").unwrap();
        // Shader modules
        let vert_shader = engine_shaders::load_or_compile_shader("shaders_compiled/man_vert.spv", "shaders/mandelbrot.vert", shaderc::ShaderKind::Vertex).unwrap();
        let vert_decoded = erupt::utils::decode_spv(&vert_shader.into_boxed_slice()).unwrap();
        let vert_shader_module_info = vk::ShaderModuleCreateInfoBuilder::new().code(&vert_decoded);
        let vert_shader_module = unsafe {logical_device.create_shader_module(&vert_shader_module_info, None)}.unwrap();
        let vert_stage_info = vk::PipelineShaderStageCreateInfoBuilder::new()
            .stage(vk::ShaderStageFlagBits::VERTEX)
            .module(vert_shader_module)
            .name(&entry_point);

        let frag_shader = engine_shaders::load_or_compile_shader("shaders_compiled/man_frag.spv", "shaders/mandelbrot.frag", shaderc::ShaderKind::Fragment).unwrap();
        let frag_decoded = erupt::utils::decode_spv(&frag_shader.into_boxed_slice()).unwrap();
        let frag_shader_module_info = vk::ShaderModuleCreateInfoBuilder::new().code(&frag_decoded);
        let frag_shader_module = unsafe {logical_device.create_shader_module(&frag_shader_module_info, None)}.unwrap();
        let frag_stage_info = vk::PipelineShaderStageCreateInfoBuilder::new()
            .stage(vk::ShaderStageFlagBits::FRAGMENT)
            .module(frag_shader_module)
            .name(&entry_point);
        
        let shader_stages = [vert_stage_info, frag_stage_info];

        // Vertex input settings (since vertices are hard-coded in the shader for now, ít is specified to take no input)
        let pipeline_vertex_input_state_info = vk::PipelineVertexInputStateCreateInfoBuilder::new();
        // Input assembly settings
        let pipeline_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfoBuilder::new()
            .topology(vk::PrimitiveTopology::TRIANGLE_STRIP)
            .primitive_restart_enable(false);
        // Viewport settings
        let viewports = [vk::ViewportBuilder::new()
            .x(0.0)
            .y(0.0)
            .width(swapchain_extent.width as f32)
            .height(swapchain_extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0)];
        let scissor_rects = [vk::Rect2DBuilder::new()
            .offset(vk::Offset2D{x: 0, y: 0})
            .extent(swapchain_extent)];
        let pipeline_viewport_state_info = vk::PipelineViewportStateCreateInfoBuilder::new()
            .viewports(&viewports)
            .scissors(&scissor_rects);
        // Rasterizer settings
        let pipeline_rasterization_state_info = vk::PipelineRasterizationStateCreateInfoBuilder::new()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::CLOCKWISE)
            .depth_bias_enable(false);
        // Multisampling settings
        let pipeline_multisample_state_info = vk::PipelineMultisampleStateCreateInfoBuilder::new()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlagBits::_1);
        // Color blending settings
        let pipeline_color_blend_attachment_states = [vk::PipelineColorBlendAttachmentStateBuilder::new()
            .color_write_mask(
                vk::ColorComponentFlags::R |
                vk::ColorComponentFlags::G |
                vk::ColorComponentFlags::B |
                vk::ColorComponentFlags::A)
            .blend_enable(false)];
        let pipeline_color_blend_state_info = vk::PipelineColorBlendStateCreateInfoBuilder::new()
            .logic_op_enable(false)
            .attachments(&pipeline_color_blend_attachment_states);
        
        // Pipeline layout
        let push_constant_ranges = [vk::PushConstantRangeBuilder::new()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .offset(0)
            .size((push_constants.len()*size_of::<f32>()) as u32)];
        let pipeline_layout_info = vk::PipelineLayoutCreateInfoBuilder::new()
            .push_constant_ranges(&push_constant_ranges);
        let pipeline_layout = unsafe {logical_device.create_pipeline_layout(&pipeline_layout_info, None)}.unwrap();
        
        let graphics_pipeline_infos = [vk::GraphicsPipelineCreateInfoBuilder::new()
            .stages(&shader_stages)
            .vertex_input_state(&pipeline_vertex_input_state_info)
            .input_assembly_state(&pipeline_input_assembly_state_info)
            .viewport_state(&pipeline_viewport_state_info)
            .rasterization_state(&pipeline_rasterization_state_info)
            .multisample_state(&pipeline_multisample_state_info)
            .color_blend_state(&pipeline_color_blend_state_info)
            .layout(pipeline_layout)
            .render_pass(renderpass)
            .subpass(0)];
        let graphics_pipeline = unsafe {logical_device.create_graphics_pipelines(vk::PipelineCache::null(), &graphics_pipeline_infos, None)}.unwrap()[0];

        //Once the graphics pipeline has been created, the SPIR-V bytecode is compiled into the pipeline itself
        //The shader modules can therefore be destroyed already
        unsafe {
            logical_device.destroy_shader_module(vert_shader_module, None);
            logical_device.destroy_shader_module(frag_shader_module, None);
        }

        (graphics_pipeline, pipeline_layout, renderpass)
    };

    //// Framebuffers
    let mut swapchain_framebuffers = Vec::new();
    for i in 0..image_views.len() {
        let attachments = [image_views[i]];

        let framebuffer_info = vk::FramebufferCreateInfoBuilder::new()
            .render_pass(renderpass)
            .attachments(&attachments)
            .width(swapchain_extent.width)
            .height(swapchain_extent.height)
            .layers(1);

        let framebuffer = unsafe {logical_device.create_framebuffer(&framebuffer_info, None)}.expect("Could not create framebuffer!");
        swapchain_framebuffers.push(framebuffer);
    }

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
        renderpass,
        graphics_pipeline,
        graphics_pipeline_layout,
        &push_constants
    );

    //// Create semaphores for in-render-pass synchronization
    let mut image_available_sems = SmallVec::with_capacity(MAX_FRAMES_IN_FLIGHT);
    let mut render_finished_sems = SmallVec::with_capacity(MAX_FRAMES_IN_FLIGHT);
    let mut in_flight_fences = SmallVec::with_capacity(MAX_FRAMES_IN_FLIGHT);
    let mut images_in_flight = SmallVec::with_capacity(swapchain_images.len());
    unsafe {
        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            image_available_sems.push(logical_device.create_semaphore(&vk::SemaphoreCreateInfoBuilder::new(), None).unwrap());
            render_finished_sems.push(logical_device.create_semaphore(&vk::SemaphoreCreateInfoBuilder::new(), None).unwrap());
            in_flight_fences.push(logical_device.create_fence(&vk::FenceCreateInfoBuilder::new().flags(vk::FenceCreateFlags::SIGNALED), None).unwrap());
        }
        for _ in 0..swapchain_images.len() {
            images_in_flight.push(vk::Fence::null());
        }
    }

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
        renderpass,
        framebuffers: swapchain_framebuffers,
        command_pool,
        command_buffers,
        image_available_sems,
        render_finished_sems,
        in_flight_fences,
        images_in_flight,
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