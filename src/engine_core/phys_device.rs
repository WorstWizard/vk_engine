use ash::{extensions::khr::Surface, vk, Instance};
use std::ffi::CStr;

pub fn query_swap_chain_support(
    surface_loader: &Surface,
    surface: &vk::SurfaceKHR,
    device: &vk::PhysicalDevice,
) -> (
    vk::SurfaceCapabilitiesKHR,
    Vec<vk::SurfaceFormatKHR>,
    Vec<vk::PresentModeKHR>,
) {
    let surface_capabilities =
        unsafe { surface_loader.get_physical_device_surface_capabilities(*device, *surface) }
            .unwrap();
    let formats =
        unsafe { surface_loader.get_physical_device_surface_formats(*device, *surface) }.unwrap();
    let present_modes =
        unsafe { surface_loader.get_physical_device_surface_present_modes(*device, *surface) }
            .unwrap();
    (
        surface_capabilities,
        formats.to_vec(),
        present_modes.to_vec(),
    )
}

/// Helper struct for queue family indices
#[derive(Clone, Copy)]
pub struct QueueFamilyIndices {
    pub graphics_queue: u32,
    pub present_queue: u32,
}
impl QueueFamilyIndices {
    /// Copies the queue indices into an array and returns it
    /// **Do not** rely on the size or order of the array, they may change
    pub fn array(&self) -> [u32; 2] {
        [self.graphics_queue, self.present_queue]
    }
}

//Find supported (command) queue families. We need certain ones for the engine to work
pub fn find_queue_families(
    instance: &Instance,
    surface_loader: &Surface,
    surface: &vk::SurfaceKHR,
    device: &vk::PhysicalDevice,
) -> Option<QueueFamilyIndices> {
    let queue_family_properties =
        unsafe { instance.get_physical_device_queue_family_properties(*device) };
    let mut indices = [None, None];
    for (i, queue_family) in queue_family_properties.iter().enumerate() {
        if indices[0].is_none() && queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
            indices[0] = Some(i as u32); //Graphics queue found, look for present queue (probably the same)
        }
        if indices[1].is_none()
            && unsafe {
                surface_loader.get_physical_device_surface_support(*device, i as u32, *surface)
            }
            .unwrap()
        {
            indices[1] = Some(i as u32); //Present queue found, look for graphics queue
        }
        if indices[0].is_some() && indices[1].is_some() {
            return Some(QueueFamilyIndices {
                graphics_queue: indices[0].unwrap(),
                present_queue: indices[1].unwrap(),
            }); //Only reached if the above for loop does not break
        }
    }
    None
}
// How good is a given physical device? Uses heuristics to rank, picks best. Also invalidates devices that won't work
pub fn device_suitability(
    instance: &Instance,
    surface_loader: &Surface,
    surface: &vk::SurfaceKHR,
    device: &vk::PhysicalDevice,
) -> u32 {
    let device_properties = unsafe { instance.get_physical_device_properties(*device) };
    let device_features = unsafe { instance.get_physical_device_features(*device) };

    let mut score = 0; //Score of 0 => entirely unsuitable
    if !check_device_extension_support(instance, device) {
        return 0;
    } //Must have extension to query swap chain
    let (_, formats, present_modes) = query_swap_chain_support(surface_loader, surface, device);
    if device_features.geometry_shader == vk::FALSE
        || formats.is_empty()
        || present_modes.is_empty()
    {
        return 0;
    }
    if find_queue_families(instance, surface_loader, surface, device).is_none() {
        return 0;
    }

    if device_properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
        score += 1000
    }
    score += device_properties.limits.max_image_dimension2_d;
    //println!("Device name: {}", unsafe {CStr::from_ptr(device_properties.device_name.as_ptr())}.to_string_lossy());

    score
}
// Physical device needs to support certain extensions
fn check_device_extension_support(instance: &Instance, device: &vk::PhysicalDevice) -> bool {
    let device_extension_properties =
        unsafe { instance.enumerate_device_extension_properties(*device) }.unwrap();
    let available_extension_names: Vec<&str> = device_extension_properties
        .iter()
        .map(|ext| {
            unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) }
                .to_str()
                .unwrap()
        })
        .collect();
    for extension in super::DEVICE_EXTS {
        let ext_name = unsafe { CStr::from_ptr(extension) }.to_str().unwrap();
        if !available_extension_names.contains(&ext_name) {
            return false;
        }
    }
    true
}
