use crate::shaders;
use ash::extensions::khr::{Surface, Swapchain};
use ash::{vk, Device, Entry, Instance};
use cstr::cstr;
use glam::*;
use std::collections::HashSet;
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::rc::Rc;
use winit::window::Window;

pub mod buffer;
mod phys_device;
mod pipeline;
mod swapchain;
mod textures;

pub use buffer::ManagedBuffer;
pub use pipeline::VertexInputDescriptors;
pub use textures::ManagedImage;

pub trait ValidIndexBufferType {}
impl ValidIndexBufferType for u16 {}
impl ValidIndexBufferType for u32 {}

//["VK_LAYER_KHRONOS_validation\0" as *const str as *const [c_char] as *const c_char];
pub const VALIDATION_LAYERS: [*const c_char; 1] = [cstr!("VK_LAYER_KHRONOS_validation").as_ptr()];
#[cfg(debug_assertions)]
pub const VALIDATION_ENABLED: bool = true;
#[cfg(not(debug_assertions))]
pub const VALIDATION_ENABLED: bool = false;

pub const DEVICE_EXTS: [*const c_char; 1] = [Swapchain::name().as_ptr()];
pub const GRAPHICS_Q_IDX: usize = 0;
pub const PRESENT_Q_IDX: usize = 1;
pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub fn init_debug_messenger_info() -> vk::DebugUtilsMessengerCreateInfoEXTBuilder<'static> {
    let messenger_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(
            //vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE_EXT |
            vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(debug_callback));

    messenger_info
}
unsafe extern "system" fn debug_callback(
    _message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    eprintln!(
        "{}",
        CStr::from_ptr((*p_callback_data).p_message).to_string_lossy()
    );
    vk::FALSE
}

pub fn check_validation_layer_support(entry: &Entry) -> bool {
    let available_layers = entry.enumerate_instance_layer_properties().unwrap();
    for layer in &VALIDATION_LAYERS {
        let mut found = false;
        for layer_properties in &available_layers {
            let layer_name_ptr = &layer_properties.layer_name[0] as *const i8;
            unsafe {
                //println!("{:?}", CStr::from_ptr(layer_name_ptr));
                if CStr::from_ptr(layer_name_ptr) == CStr::from_ptr(*layer) {
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return false;
        }
    }
    true
}

pub fn find_physical_device(
    instance: &Instance,
    surface_loader: &Surface,
    surface: &vk::SurfaceKHR,
) -> (vk::PhysicalDevice, phys_device::QueueFamilyIndices) {
    let devices = unsafe { instance.enumerate_physical_devices() }.unwrap();
    if devices.is_empty() {
        panic!("No devices with Vulkan support!")
    }

    let mut suitability = 0;
    let physical_device = devices
        .into_iter()
        .max_by_key(|device| {
            suitability =
                phys_device::device_suitability(instance, surface_loader, surface, device);
            suitability
        })
        .expect("No suitable GPU could be found!");
    if suitability == 0 {
        panic!("No suitable GPU could be found!")
    }

    let queue_family_indices =
        phys_device::find_queue_families(instance, surface_loader, surface, &physical_device)
            .unwrap(); //Checked in device_suitabiliy, so will always succeed
    (physical_device, queue_family_indices)
}

pub fn create_logical_device(
    instance: &Instance,
    physical_device: &vk::PhysicalDevice,
    queue_family_indices: phys_device::QueueFamilyIndices,
) -> Rc<Device> {
    let unique_queue_family_indices: Vec<u32> = HashSet::from(queue_family_indices.array())
        .drain()
        .collect();
    let device_queue_infos: &[vk::DeviceQueueCreateInfo] = &unique_queue_family_indices
        .into_iter()
        .map(|index| {
            *vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(index)
                .queue_priorities(&[1.0])
        })
        .collect::<Vec<vk::DeviceQueueCreateInfo>>()
        .into_boxed_slice();

    let device_features = vk::PhysicalDeviceFeatures::builder().sampler_anisotropy(true);
    let device_create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(device_queue_infos)
        .enabled_features(&device_features)
        .enabled_extension_names(&DEVICE_EXTS);

    Rc::new(
        unsafe { instance.create_device(*physical_device, &device_create_info, None) }
            .expect("Failed to create logical device!"),
    )
}

pub fn get_queue_handles(
    logical_device: &Device,
    queue_family_indices: phys_device::QueueFamilyIndices,
) -> (vk::Queue, vk::Queue) {
    let graphics_queue =
        unsafe { logical_device.get_device_queue(queue_family_indices.graphics_queue, 0) };
    let present_queue =
        unsafe { logical_device.get_device_queue(queue_family_indices.present_queue, 0) };
    (graphics_queue, present_queue)
}

pub fn create_swapchain(
    window: &Window,
    surface_loader: &Surface,
    surface: &vk::SurfaceKHR,
    physical_device: &vk::PhysicalDevice,
    swapchain_loader: &Swapchain,
    queue_family_indices: phys_device::QueueFamilyIndices,
) -> (vk::SwapchainKHR, vk::Format, vk::Extent2D, Vec<vk::Image>) {
    let (surface_capabilities, formats, present_modes) =
        phys_device::query_swap_chain_support(surface_loader, surface, physical_device);
    let surface_format = swapchain::choose_swap_surface_format(&formats);
    let present_mode =
        swapchain::choose_swap_present_mode(&present_modes, vk::PresentModeKHR::MAILBOX);
    let swap_extent = swapchain::choose_swap_extent(window, &surface_capabilities);
    let image_count = {
        //Pick smaller value between minimum + 1 and the maximum
        let mut count = surface_capabilities.min_image_count + 1;
        if surface_capabilities.max_image_count > 0 && count > surface_capabilities.max_image_count
        {
            count = surface_capabilities.max_image_count
        }
        count
    };
    let mut swapchain_info = vk::SwapchainCreateInfoKHR::builder()
        //Defined from above values v v v
        .surface(*surface)
        .min_image_count(image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(swap_extent)
        .present_mode(present_mode)
        //Should never change v v v
        .image_array_layers(1)
        .pre_transform(surface_capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .clipped(true)
        //Might change depending on use case v v v
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT);

    let indices = queue_family_indices.array();
    if queue_family_indices.graphics_queue != queue_family_indices.present_queue {
        swapchain_info = swapchain_info
            .image_sharing_mode(vk::SharingMode::CONCURRENT)
            .queue_family_indices(&indices);
    } else {
        swapchain_info = swapchain_info.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
    }
    let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_info, None) }
        .expect("Could not create swapchain!");
    let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain) }.unwrap();

    (
        swapchain,
        surface_format.format,
        swap_extent,
        swapchain_images,
    )
}

pub fn create_swapchain_image_views(
    logical_device: &Device,
    swapchain_images: &Vec<vk::Image>,
    image_format: vk::Format,
) -> Vec<vk::ImageView> {
    let mut image_views = Vec::new();
    for swap_im in swapchain_images {
        // let image_view_info = vk::ImageViewCreateInfo::builder()
        //     .image(*swap_im)
        //     .view_type(vk::ImageViewType::TYPE_2D)
        //     .format(image_format)
        //     .components(vk::ComponentMapping {
        //         r: vk::ComponentSwizzle::IDENTITY,
        //         g: vk::ComponentSwizzle::IDENTITY,
        //         b: vk::ComponentSwizzle::IDENTITY,
        //         a: vk::ComponentSwizzle::IDENTITY,
        //     })
        //     .subresource_range(vk::ImageSubresourceRange {
        //         aspect_mask: vk::ImageAspectFlags::COLOR,
        //         base_mip_level: 0,
        //         level_count: 1,
        //         base_array_layer: 0,
        //         layer_count: 1,
        //     });
        // let image_view =
        //     unsafe { logical_device.create_image_view(&image_view_info, None) }.unwrap();
        // image_views.push(image_view);
        image_views.push(textures::create_image_view(
            logical_device,
            *swap_im,
            image_format,
            vk::ImageAspectFlags::COLOR,
        ))
    }
    image_views
}

pub fn create_graphics_pipeline(
    logical_device: &Device,
    swapchain_extent: vk::Extent2D,
    image_format: vk::Format,
    shaders: &Vec<shaders::Shader>,
    vertex_input_descriptors: &VertexInputDescriptors,
    descriptor_set_bindings: Vec<vk::DescriptorSetLayoutBinding>,
    push_constants: [f32; 1],
) -> (
    vk::Pipeline,
    vk::PipelineLayout,
    vk::DescriptorSetLayout,
    vk::RenderPass,
) {
    let render_pass = pipeline::default_render_pass(logical_device, image_format);

    let pipeline = pipeline::default_pipeline(
        logical_device,
        render_pass,
        swapchain_extent,
        shaders,
        vertex_input_descriptors,
        descriptor_set_bindings,
        push_constants,
    );
    (pipeline.0, pipeline.1, pipeline.2, render_pass)
}

pub fn create_framebuffers(
    logical_device: &Device,
    render_pass: vk::RenderPass,
    swapchain_extent: vk::Extent2D,
    image_views: &[vk::ImageView],
    depth_image_view: vk::ImageView,
) -> Vec<vk::Framebuffer> {
    let mut swapchain_framebuffers = Vec::new();
    for im_view in image_views {
        let attachments = [*im_view, depth_image_view];

        let framebuffer_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass)
            .attachments(&attachments)
            .width(swapchain_extent.width)
            .height(swapchain_extent.height)
            .layers(1);

        let framebuffer = unsafe { logical_device.create_framebuffer(&framebuffer_info, None) }
            .expect("Could not create framebuffer!");
        swapchain_framebuffers.push(framebuffer);
    }
    swapchain_framebuffers
}

pub struct SyncPrims {
    pub image_available: Vec<vk::Semaphore>,
    pub render_finished: Vec<vk::Semaphore>,
    pub in_flight: Vec<vk::Fence>,
}
pub fn create_sync_primitives(logical_device: &Device) -> SyncPrims {
    let mut image_available = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
    let mut render_finished = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
    let mut in_flight = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
    unsafe {
        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            image_available.push(
                logical_device
                    .create_semaphore(&vk::SemaphoreCreateInfo::builder(), None)
                    .unwrap(),
            );
            render_finished.push(
                logical_device
                    .create_semaphore(&vk::SemaphoreCreateInfo::builder(), None)
                    .unwrap(),
            );
            in_flight.push(
                logical_device
                    .create_fence(
                        &vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED),
                        None,
                    )
                    .unwrap(),
            );
        }
    }
    SyncPrims {
        image_available,
        render_finished,
        in_flight,
    }
}

pub fn allocate_command_buffers(
    logical_device: &Device,
    command_pool: vk::CommandPool,
    amount: u32,
) -> Vec<vk::CommandBuffer> {
    let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(amount);
    unsafe { logical_device.allocate_command_buffers(&command_buffer_allocate_info) }
        .expect("Could not create command buffers!")
}

pub fn create_staging_buffer(
    instance: &Instance,
    physical_device: &vk::PhysicalDevice,
    logical_device: &Rc<Device>,
    memory_size: vk::DeviceSize,
) -> ManagedBuffer {
    //Host visible buffer; data is transferred to a device local buffer at transfer stage
    let staging_buffer = buffer::create_buffer(
        logical_device,
        memory_size,
        vk::BufferUsageFlags::TRANSFER_SRC,
    );
    let staging_buffer_memory = buffer::allocate_and_bind_buffer(
        instance,
        physical_device,
        logical_device,
        staging_buffer,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    );

    ManagedBuffer {
        logical_device: Rc::clone(logical_device),
        // memory_size,
        buffer: staging_buffer,
        buffer_memory: Some(staging_buffer_memory),
        memory_ptr: None,
    }
}

pub fn create_vertex_buffer(
    instance: &Instance,
    physical_device: &vk::PhysicalDevice,
    logical_device: &Rc<Device>,
    memory_size: u64,
) -> ManagedBuffer {
    //Device local buffer, or *true* vertex buffer, needs a staging buffer to transfer data to it
    let vertex_buffer = buffer::create_buffer(
        logical_device,
        memory_size,
        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
    );
    let vertex_buffer_memory = buffer::allocate_and_bind_buffer(
        instance,
        physical_device,
        logical_device,
        vertex_buffer,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    );

    ManagedBuffer {
        logical_device: Rc::clone(logical_device),
        // memory_size,
        buffer: vertex_buffer,
        buffer_memory: Some(vertex_buffer_memory),
        memory_ptr: None,
    }
}

pub fn create_index_buffer<IndexType: ValidIndexBufferType>(
    instance: &Instance,
    physical_device: &vk::PhysicalDevice,
    logical_device: &Rc<Device>,
    count: usize,
) -> ManagedBuffer {
    //Easy to get the memory size wrong, might fail invisibly
    let memory_size = (std::mem::size_of::<IndexType>() * count) as u64;
    let index_buffer = buffer::create_buffer(
        logical_device,
        memory_size,
        vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
    );
    let index_buffer_memory = buffer::allocate_and_bind_buffer(
        instance,
        physical_device,
        logical_device,
        index_buffer,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    );

    ManagedBuffer {
        logical_device: Rc::clone(logical_device),
        // memory_size,
        buffer: index_buffer,
        buffer_memory: Some(index_buffer_memory),
        memory_ptr: None,
    }
}

pub fn create_uniform_buffers(
    instance: &Instance,
    physical_device: &vk::PhysicalDevice,
    logical_device: &Rc<Device>,
    memory_size: u64,
    count: usize,
) -> Vec<ManagedBuffer> {
    //Easy to get the memory size wrong, might fail invisibly
    let mut uniform_buffers = Vec::with_capacity(count);
    for _ in 0..count {
        let uniform_buffer = buffer::create_buffer(
            logical_device,
            memory_size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
        );
        let uniform_buffer_memory = buffer::allocate_and_bind_buffer(
            instance,
            physical_device,
            logical_device,
            uniform_buffer,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );

        let mut managed_buffer = ManagedBuffer {
            logical_device: Rc::clone(logical_device),
            // memory_size,
            buffer: uniform_buffer,
            buffer_memory: Some(uniform_buffer_memory),
            memory_ptr: None,
        };
        managed_buffer.map_buffer_memory(); // Map immediately, as the uniform buffers are persistently mapped

        uniform_buffers.push(managed_buffer);
    }
    uniform_buffers
}

pub fn create_image(
    instance: &Instance,
    physical_device: &vk::PhysicalDevice,
    logical_device: &Rc<Device>,
    format: vk::Format,
    tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags,
    aspect_flags: vk::ImageAspectFlags,
    dimensions: (u32, u32),
) -> ManagedImage {
    let texture_image = textures::create_image(logical_device, format, tiling, usage, dimensions);
    let image_memory = Some(textures::allocate_and_bind_image(
        instance,
        physical_device,
        logical_device,
        texture_image,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    ));
    let texture_image_view =
        textures::create_image_view(logical_device, texture_image, format, aspect_flags);
    let managed_image = ManagedImage {
        logical_device: Rc::clone(logical_device),
        image: texture_image,
        image_view: texture_image_view,
        image_memory,
        memory_ptr: None,
    };
    managed_image
}

/// # Safety
/// The memory pointed to by `buffer_pointer` must have at least as much space allocated as is required by `data`, and `buffer_pointer` must be valid.
pub unsafe fn write_vec_to_buffer<T: Sized>(buffer_pointer: *mut c_void, data: &Vec<T>) {
    std::ptr::copy_nonoverlapping(data.as_ptr(), buffer_pointer as *mut T, data.len());
}

pub unsafe fn write_struct_to_buffer<T: Sized>(buffer_pointer: *mut c_void, data: *const T) {
    std::ptr::copy_nonoverlapping(data, buffer_pointer as *mut T, 1);
}

/// Immediately submits the given commands to the given queue. Blocks until completion.
pub unsafe fn immediate_commands<F: FnOnce(vk::CommandBuffer) -> ()>(
    logical_device: &Device,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    commands: F,
) {
    let temp_command_buffers = [allocate_command_buffers(logical_device, command_pool, 1)[0]];
    let recording_info =
        vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    logical_device
        .begin_command_buffer(temp_command_buffers[0], &recording_info)
        .unwrap();

    commands(temp_command_buffers[0]);

    logical_device
        .end_command_buffer(temp_command_buffers[0])
        .unwrap();

    let submit_info = vk::SubmitInfo::builder().command_buffers(&temp_command_buffers);
    logical_device
        .queue_submit(queue, &[*submit_info], vk::Fence::null())
        .unwrap();
    logical_device.queue_wait_idle(queue).unwrap();
    logical_device.free_command_buffers(command_pool, &temp_command_buffers);
}

/// Immediately sends command to a queue to copy data from src buffer to dst buffer. Blocks until transfer completes.
pub fn copy_buffer(
    logical_device: &Device,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    src_buffer: vk::Buffer,
    dst_buffer: vk::Buffer,
    memory_size: vk::DeviceSize,
) {
    unsafe {
        immediate_commands(logical_device, command_pool, queue, |cmd_buffer| {
            let copy_region = vk::BufferCopy::builder()
                .src_offset(0)
                .dst_offset(0)
                .size(memory_size);
            logical_device.cmd_copy_buffer(cmd_buffer, src_buffer, dst_buffer, &[*copy_region]);
        })
    }
}
