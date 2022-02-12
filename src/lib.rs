use winit::event::{Event, WindowEvent, VirtualKeyCode};
use winit::window::{Window, WindowBuilder};
use winit::event_loop::{EventLoop, ControlFlow};

use erupt::{vk, {EntryLoader, InstanceLoader, DeviceLoader}, {ExtendableFrom, SmallVec}, utils::{surface}, cstr};

use std::ffi::{CString, CStr};
use std::os::raw::{c_char, c_void};
use std::collections::HashSet;
use std::mem::size_of;
use std::time;

const HEIGHT: u32 = 800;
const WIDTH: u32 = 800;
const APP_TITLE: &str = "Mandelbrot in Vulkan - Kristian Knudsen";


// Shaders
const VERT_SHADER: &[u8] = include_bytes!("man_vert.spv");
const FRAG_SHADER: &[u8] = include_bytes!("man_frag.spv");


const VALIDATION_LAYERS: [*const c_char; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];
#[cfg(debug_assertions)]
const VALIDATION_ENABLED: bool = true;
#[cfg(not(debug_assertions))]
const VALIDATION_ENABLED: bool = false;


const MAX_FRAMES_IN_FLIGHT: usize = 2;



unsafe extern "system" fn debug_callback(
    _message_severity: vk::DebugUtilsMessageSeverityFlagBitsEXT,
    _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void
) -> vk::Bool32 {
    eprintln!("{}", CStr::from_ptr((*p_callback_data).p_message).to_string_lossy());
    vk::FALSE
}

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
    pub messenger: vk::DebugUtilsMessengerEXT,
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
            if !self.messenger.is_null() {
                self.instance.destroy_debug_utils_messenger_ext(self.messenger, None)
            }
            self.instance.destroy_surface_khr(self.surface, None);
            self.instance.destroy_instance(None);
        }
        println!("VulkanApp dropped succesfully");
    }
}
pub fn init_vulkan(window: &Window) -> VulkanApp {
    let entry = Box::new(EntryLoader::new().unwrap());

    fn check_validation_layer_support(entry: &EntryLoader) -> bool{
        let available_layers = unsafe {entry.enumerate_instance_layer_properties(None).unwrap()};
        for layer in &VALIDATION_LAYERS {
            let mut found = false;
            for layer_properties in &available_layers {
                let layer_name_ptr = &layer_properties.layer_name[0] as *const i8;
                unsafe {
                    //println!("{:?}", CStr::from_ptr(layer_name_ptr));
                    if CStr::from_ptr(layer_name_ptr) == CStr::from_ptr(*layer) {
                        found = true; break
                    }
                }
            }
            if !found {return false}
        }
        return true
    }

    if VALIDATION_ENABLED && !check_validation_layer_support(&entry) {
        panic!("Validation layer requested but not available!");
    }

    //// Application info
    let app_name = CString::new("Hello Triangle").unwrap();
    let engine_name = CString::new("No Engine").unwrap();

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

    //// Instance info & debug messenger
    let mut messenger_info = init_debug_messenger_info();
    let mut instance_info = vk::InstanceCreateInfoBuilder::new()
        .application_info(&app_info)
        .enabled_extension_names(&instance_extensions);
    if VALIDATION_ENABLED {
        instance_info = instance_info
            .enabled_layer_names(&VALIDATION_LAYERS)
            .extend_from(&mut messenger_info);
    }
    
    //// Instance created
    let instance = Box::new(unsafe {InstanceLoader::new(&entry, &instance_info)}.expect("Failed to create Vulkan instance!"));
    // Messenger attached
    let messenger = if VALIDATION_ENABLED {
        unsafe {instance.create_debug_utils_messenger_ext(&messenger_info, None)}.unwrap()
    } else {
        vk::DebugUtilsMessengerEXT::default()
    };

    //// Window surface creation
    let surface = unsafe { surface::create_surface(&instance, &window, None) }.unwrap();

    //// Physical device and queues
    const GRAPHICS_Q_IDX: usize = 0;
    const PRESENT_Q_IDX: usize = 1;
    const DEVICE_EXTS: [*const c_char; 1] = [vk::KHR_SWAPCHAIN_EXTENSION_NAME];

    // Swapchain queries
    fn query_swap_chain_support(device: &vk::PhysicalDevice, surface: &vk::SurfaceKHR, instance: &InstanceLoader)
    -> (vk::SurfaceCapabilitiesKHR, Vec<vk::SurfaceFormatKHR>, Vec<vk::PresentModeKHR>) {
            let surface_capabilities = unsafe {instance.get_physical_device_surface_capabilities_khr(*device, *surface)}.unwrap();
            let formats = unsafe {instance.get_physical_device_surface_formats_khr(*device, *surface, None)}.unwrap();
            let present_modes = unsafe {instance.get_physical_device_surface_present_modes_khr(*device, *surface, None)}.unwrap();
            (surface_capabilities, formats.to_vec(), present_modes.to_vec())
    }

    let (physical_device, queue_family_indices) = {
        fn find_queue_families(device: &vk::PhysicalDevice, surface: &vk::SurfaceKHR, instance: &InstanceLoader) -> Option<[u32; 2]> {
            let queue_family_properties = unsafe {instance.get_physical_device_queue_family_properties(*device, None)};
            let mut indices = [0; 2];
            let mut found_queues = [false; 2];
            'outer:
            for (i, queue_family) in queue_family_properties.iter().enumerate() {
                if !found_queues[GRAPHICS_Q_IDX] && queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    indices[GRAPHICS_Q_IDX] = i as u32; //Graphics queue found, look for present queue (probably the same)
                    found_queues[GRAPHICS_Q_IDX] = true;
                }
                if !found_queues[PRESENT_Q_IDX] && unsafe {instance.get_physical_device_surface_support_khr(*device, i as u32, *surface)}.unwrap() {
                    indices[PRESENT_Q_IDX] = i as u32; //Graphics queue found, look for present queue (probably the same)
                    found_queues[PRESENT_Q_IDX] = true;
                }
                for queue_found in found_queues {
                    if !queue_found {break 'outer}
                }
                return Some(indices) //Only reached if the above for loop does not break
            }
            None
        }
        
        fn check_device_extension_support(device: &vk::PhysicalDevice, instance: &InstanceLoader) -> bool {
            let device_extension_properties = unsafe {instance.enumerate_device_extension_properties(*device, None, None)}.unwrap();
            let available_extension_names: Vec<&str> = device_extension_properties
                .iter()
                .map(|ext| unsafe {CStr::from_ptr(ext.extension_name.as_ptr())}.to_str().unwrap() ).collect();
            for extension in DEVICE_EXTS {
                let ext_name = unsafe {CStr::from_ptr(extension)}.to_str().unwrap();
                if !available_extension_names.contains(&ext_name) {
                    return false
                }
            }
            return true
        }

        //Checking device suitability
        let devices = unsafe {instance.enumerate_physical_devices(None)}.unwrap();
        if devices.len() == 0 {panic!("No devices with Vulkan support!")}
        fn is_device_suitable(device: &vk::PhysicalDevice, surface: &vk::SurfaceKHR, instance: &InstanceLoader) -> bool {
            let device_properties = unsafe {instance.get_physical_device_properties(*device)};
            let device_features = unsafe {instance.get_physical_device_features(*device)};
            println!("Device name: {}", unsafe {CStr::from_ptr(device_properties.device_name.as_ptr())}.to_string_lossy());

            if !check_device_extension_support(device, instance) {return false} //Must have extension to query swap chain
            let (_, formats, present_modes) = query_swap_chain_support(device, surface, instance);

            return device_features.geometry_shader == vk::TRUE
                && if let Some(_) = find_queue_families(device, surface, instance) {true} else {false}
                && !formats.is_empty() && !present_modes.is_empty()
                
        }
        fn rate_device_suitability(device_properties: &vk::PhysicalDeviceProperties, device_features: &vk::PhysicalDeviceFeatures) -> u32 {
            let mut score = 0;
            if device_properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {score += 1000}
            score += device_properties.limits.max_image_dimension2_d;
            if device_features.geometry_shader == vk::FALSE {return 0}
            return score
        }
    
        //Picking device
        let physical_device = devices.into_iter().max_by_key(
            |device| {
                let device_properties = unsafe {instance.get_physical_device_properties(*device)};
                let device_features = unsafe {instance.get_physical_device_features(*device)};
                rate_device_suitability(&device_properties, &device_features)
            }
        ).expect("No devices could be found!");
        if !is_device_suitable(&physical_device, &surface, &instance) {panic!("No suitable GPU found!")}
        let queue_family_indices = find_queue_families(&physical_device, &surface, &instance).unwrap();
        
        (physical_device, queue_family_indices)
    };

    //// Logical device
    let unique_queue_family_indices: Vec<u32> = HashSet::from(queue_family_indices.clone()).into_iter().collect();
    let device_queue_infos: &[vk::DeviceQueueCreateInfoBuilder] = &unique_queue_family_indices.into_iter().map(|index| {
        vk::DeviceQueueCreateInfoBuilder::new()
        .queue_family_index(index)
        .queue_priorities(&[1.0])
    }).collect::<Vec<vk::DeviceQueueCreateInfoBuilder>>().into_boxed_slice();
    
    let device_features = vk::PhysicalDeviceFeatures::default();
    let mut device_create_info = vk::DeviceCreateInfoBuilder::new()
        .queue_create_infos(device_queue_infos)
        .enabled_features(&device_features)
        .enabled_extension_names(&DEVICE_EXTS);
    if VALIDATION_ENABLED {
        device_create_info = device_create_info.enabled_layer_names(&VALIDATION_LAYERS);
    }
    let logical_device = Box::new(unsafe {DeviceLoader::new(&instance, physical_device, &device_create_info)}.expect("Failed to create logical device!"));

    //// Queue handles
    let graphics_queue = unsafe {logical_device.get_device_queue(queue_family_indices[GRAPHICS_Q_IDX], 0)};
    let present_queue = unsafe {logical_device.get_device_queue(queue_family_indices[PRESENT_Q_IDX], 0)};


    //// Picking swapchain settings
    fn choose_swap_surface_format(formats: &Vec<vk::SurfaceFormatKHR>) -> vk::SurfaceFormatKHR {
        for available_format in formats {
            if available_format.format == vk::Format::R8G8B8A8_SRGB && available_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR_KHR {
                return *available_format
            }
        }
        return formats[0];
    }
    fn choose_swap_present_mode(present_modes: &Vec<vk::PresentModeKHR>) -> vk::PresentModeKHR {
        for available_mode in present_modes {
            if *available_mode == vk::PresentModeKHR::MAILBOX_KHR {
                return *available_mode
            }
        }
        return vk::PresentModeKHR::FIFO_KHR;
    }
    fn choose_swap_extent(capabilities: &vk::SurfaceCapabilitiesKHR, window: &Window) -> vk::Extent2D {
        //If width/height of current extent is u32::MAX, the window manager allows selecting an extent different from the window resolution
        if capabilities.current_extent.width != u32::MAX { //Extent is specified already, must use it
            return capabilities.current_extent
        } else {
            let window_size = window.inner_size();
            let mut actual_extent = vk::Extent2D{width: window_size.width, height: window_size.height};
            actual_extent.width = actual_extent.width.clamp(capabilities.min_image_extent.width, capabilities.max_image_extent.width);
            actual_extent.height = actual_extent.height.clamp(capabilities.min_image_extent.height, capabilities.max_image_extent.height);
            return actual_extent;
        }
    }

    //// Creating swapchain
    let (swapchain, image_format, swapchain_extent) = {
        let (surface_capabilities, formats, present_modes) = query_swap_chain_support(&physical_device, &surface, &instance);
        let surface_format = choose_swap_surface_format(&formats);
        let present_mode = choose_swap_present_mode(&present_modes);
        let swap_extent = choose_swap_extent(&surface_capabilities, &window);
        let image_count = {
            let mut count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.min_image_count > 0 && count > surface_capabilities.max_image_count {count = surface_capabilities.max_image_count}
            count
        };
        let mut swapchain_info = vk::SwapchainCreateInfoKHRBuilder::new()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(swap_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .composite_alpha(vk::CompositeAlphaFlagBitsKHR::OPAQUE_KHR)
            .pre_transform(surface_capabilities.current_transform)
            .present_mode(present_mode)
            .clipped(true);
        if queue_family_indices[GRAPHICS_Q_IDX] != queue_family_indices[PRESENT_Q_IDX] {
            swapchain_info = swapchain_info.image_sharing_mode(vk::SharingMode::CONCURRENT).queue_family_indices(&queue_family_indices);
        } else {
            swapchain_info = swapchain_info.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
        }
        let swapchain = unsafe {logical_device.create_swapchain_khr(&swapchain_info, None)}.expect("Could not create swapchain!");

        (swapchain, surface_format.format, swap_extent)
    };
    let swapchain_images = unsafe {logical_device.get_swapchain_images_khr(swapchain, None)}.unwrap();

    //// Image views
    let mut image_views = Vec::new();
    for i in 0..swapchain_images.len() {
        let image_view_info = vk::ImageViewCreateInfoBuilder::new()
            .image(swapchain_images[i])
            .view_type(vk::ImageViewType::_2D)
            .format(image_format)
            .components(vk::ComponentMapping{
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY,
            }).
            subresource_range(vk::ImageSubresourceRange{
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let image_view = unsafe {logical_device.create_image_view(&image_view_info, None)}.unwrap();
        image_views.push(image_view);
    }

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
        let vert_decoded = erupt::utils::decode_spv(VERT_SHADER).unwrap();
        let vert_shader_module_info = vk::ShaderModuleCreateInfoBuilder::new().code(&vert_decoded);
        let vert_shader_module = unsafe {logical_device.create_shader_module(&vert_shader_module_info, None)}.unwrap();
        let vert_stage_info = vk::PipelineShaderStageCreateInfoBuilder::new()
            .stage(vk::ShaderStageFlagBits::VERTEX)
            .module(vert_shader_module)
            .name(&entry_point);

        let frag_decoded = erupt::utils::decode_spv(FRAG_SHADER).unwrap();
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
        .queue_family_index(queue_family_indices[GRAPHICS_Q_IDX]);
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
        messenger,
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
    .pfn_user_callback(Some(debug_callback));
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