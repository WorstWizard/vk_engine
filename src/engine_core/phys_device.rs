use erupt::{vk, InstanceLoader};
use std::ffi::{CStr};

const GRAPHICS_Q_IDX: usize = super::GRAPHICS_Q_IDX; //Bad: The queue indices must be 0 and 1, but aren't defined here. Should be dynamic instead.
const PRESENT_Q_IDX: usize = super::PRESENT_Q_IDX;

pub fn query_swap_chain_support(instance: &InstanceLoader, surface: &vk::SurfaceKHR, device: &vk::PhysicalDevice)
-> (vk::SurfaceCapabilitiesKHR, Vec<vk::SurfaceFormatKHR>, Vec<vk::PresentModeKHR>) {
        let surface_capabilities = unsafe {instance.get_physical_device_surface_capabilities_khr(*device, *surface)}.unwrap();
        let formats = unsafe {instance.get_physical_device_surface_formats_khr(*device, *surface, None)}.unwrap();
        let present_modes = unsafe {instance.get_physical_device_surface_present_modes_khr(*device, *surface, None)}.unwrap();
        (surface_capabilities, formats.to_vec(), present_modes.to_vec())
}
//Find supported (command) queue families. We need certain ones for the engine to work
pub fn find_queue_families(instance: &InstanceLoader, surface: &vk::SurfaceKHR, device: &vk::PhysicalDevice) -> Option<[u32; 2]> {
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
            indices[PRESENT_Q_IDX] = i as u32; //Present queue found, look for graphics queue
            found_queues[PRESENT_Q_IDX] = true;
        }
        for queue_found in &found_queues {
            if !queue_found {break 'outer}
        }
        return Some(indices) //Only reached if the above for loop does not break
    }
    None
}
// How good is a given physical device? Uses heuristics to rank, picks best. Also invalidates devices that won't work
pub fn device_suitability(instance: &InstanceLoader, surface: &vk::SurfaceKHR, device: &vk::PhysicalDevice) -> u32 {
    let device_properties = unsafe {instance.get_physical_device_properties(*device)};
    let device_features = unsafe {instance.get_physical_device_features(*device)};

    let mut score = 0; //Score of 0 => entirely unsuitable
    if !check_device_extension_support(instance, device) {return 0} //Must have extension to query swap chain
    let (_, formats, present_modes) = query_swap_chain_support(instance, surface, device);
    if device_features.geometry_shader == vk::FALSE || formats.is_empty() || present_modes.is_empty() {return 0}
    if let None = find_queue_families(instance, surface, device) {return 0}

    if device_properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {score += 1000}
    score += device_properties.limits.max_image_dimension2_d;
    //println!("Device name: {}", unsafe {CStr::from_ptr(device_properties.device_name.as_ptr())}.to_string_lossy());

    return score
}
// Physical device needs to support certain extensions
fn check_device_extension_support(instance: &InstanceLoader, device: &vk::PhysicalDevice) -> bool {
    let device_extension_properties = unsafe {instance.enumerate_device_extension_properties(*device, None, None)}.unwrap();
    let available_extension_names: Vec<&str> = device_extension_properties
        .iter()
        .map(|ext| unsafe {CStr::from_ptr(ext.extension_name.as_ptr())}.to_str().unwrap() ).collect();
    for extension in super::DEVICE_EXTS.iter() {
        let ext_name = unsafe {CStr::from_ptr(*extension)}.to_str().unwrap();
        if !available_extension_names.contains(&ext_name) {
            return false
        }
    }
    return true
}