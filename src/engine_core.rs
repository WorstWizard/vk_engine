use std::ffi::{CStr};
use std::os::raw::{c_void, c_char};
use std::collections::HashSet;
use winit::window::Window;
use erupt::{vk, EntryLoader, InstanceLoader, DeviceLoader};
use erupt::cstr;

mod phys_device;
mod swapchain;

pub const VALIDATION_LAYERS: [*const c_char; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];
#[cfg(debug_assertions)]
pub const VALIDATION_ENABLED: bool = true;
#[cfg(not(debug_assertions))]
pub const VALIDATION_ENABLED: bool = false;

pub const DEVICE_EXTS: [*const c_char; 1] = [vk::KHR_SWAPCHAIN_EXTENSION_NAME];
pub const GRAPHICS_Q_IDX: usize = 0;
pub const PRESENT_Q_IDX: usize = 1;

pub unsafe extern "system" fn debug_callback(
    _message_severity: vk::DebugUtilsMessageSeverityFlagBitsEXT,
    _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void
) -> vk::Bool32 {
    eprintln!("{}", CStr::from_ptr((*p_callback_data).p_message).to_string_lossy());
    vk::FALSE
}

pub fn check_validation_layer_support(entry: &EntryLoader) -> bool{
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

pub fn find_physical_device(instance: &InstanceLoader, surface: &vk::SurfaceKHR) -> (vk::PhysicalDevice, [u32;2]) {
    let devices = unsafe {instance.enumerate_physical_devices(None)}.unwrap();
    if devices.len() == 0 {panic!("No devices with Vulkan support!")}

    let mut suitability = 0;
    let physical_device = devices.into_iter().max_by_key(
    |device| {
        suitability = phys_device::device_suitability(instance, surface, &device)
    }
    ).expect("No suitable GPU could be found!");
    if suitability <= 0 {panic!("No suitable GPU could be found!")}

    let queue_family_indices = phys_device::find_queue_families(instance, surface, &physical_device).unwrap(); //Checked in device_suitabiliy, so will always succeed
    (physical_device, queue_family_indices)
}

pub fn create_logical_device(instance: &InstanceLoader, physical_device: &vk::PhysicalDevice, queue_family_indices: [u32; 2]) -> Box<DeviceLoader> {
    let unique_queue_family_indices: Vec<u32> = HashSet::from(queue_family_indices).into_iter().collect();
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
    let logical_device = Box::new(unsafe {DeviceLoader::new(&instance, *physical_device, &device_create_info)}.expect("Failed to create logical device!"));
    logical_device
}

pub fn get_queue_handles(logical_device: &DeviceLoader, queue_family_indices: [u32; 2]) -> (vk::Queue, vk::Queue) {
    let graphics_queue = unsafe {logical_device.get_device_queue(queue_family_indices[GRAPHICS_Q_IDX], 0)};
    let present_queue = unsafe {logical_device.get_device_queue(queue_family_indices[PRESENT_Q_IDX], 0)};
    (graphics_queue, present_queue)
}

pub fn create_swapchain(
    instance: &InstanceLoader,
    window: &Window,
    surface: &vk::SurfaceKHR,
    physical_device: &vk::PhysicalDevice,
    logical_device: &DeviceLoader,
    queue_family_indices: [u32; 2]
) -> (vk::SwapchainKHR, vk::Format, vk::Extent2D, erupt::SmallVec<vk::Image>) {

    let (surface_capabilities, formats, present_modes) = phys_device::query_swap_chain_support(instance, surface, physical_device);
    let surface_format = swapchain::choose_swap_surface_format(&formats);
    let present_mode = swapchain::choose_swap_present_mode(&present_modes, vk::PresentModeKHR::MAILBOX_KHR);
    let swap_extent = swapchain::choose_swap_extent(window, &surface_capabilities);
    let image_count = { //Pick smaller value between minimum + 1 and the maximum
        let mut count = surface_capabilities.min_image_count + 1;
        if surface_capabilities.max_image_count > 0 && count > surface_capabilities.max_image_count {count = surface_capabilities.max_image_count}
        count
    };
    let mut swapchain_info = vk::SwapchainCreateInfoKHRBuilder::new()
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
        .composite_alpha(vk::CompositeAlphaFlagBitsKHR::OPAQUE_KHR)
        .clipped(true)
        //Might change depending on use case v v v
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT);
    
    if queue_family_indices[GRAPHICS_Q_IDX] != queue_family_indices[PRESENT_Q_IDX] {
        swapchain_info = swapchain_info.image_sharing_mode(vk::SharingMode::CONCURRENT).queue_family_indices(&queue_family_indices);
    } else {
        swapchain_info = swapchain_info.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
    }
    let swapchain = unsafe {logical_device.create_swapchain_khr(&swapchain_info, None)}.expect("Could not create swapchain!");
    let swapchain_images = unsafe {logical_device.get_swapchain_images_khr(swapchain, None)}.unwrap();

    (swapchain, surface_format.format, swap_extent, swapchain_images)
}

pub fn create_image_views(logical_device: &DeviceLoader, swapchain_images: &erupt::SmallVec<vk::Image>, image_format: vk::Format) -> Vec<vk::ImageView> {
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
    image_views
}