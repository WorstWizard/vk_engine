use std::ffi::{CStr};
use std::os::raw::{c_void, c_char};
use erupt::{vk, InstanceLoader};

pub mod device_utils;

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

pub fn find_physical_device(instance: &InstanceLoader, surface: &vk::SurfaceKHR) -> vk::PhysicalDevice {
    let devices = unsafe {instance.enumerate_physical_devices(None)}.unwrap();
    if devices.len() == 0 {panic!("No devices with Vulkan support!")}

    let mut suitability = 0;
    let physical_device = devices.into_iter().max_by_key(
    |device| {
        suitability = device_utils::device_suitability(&instance, &surface, &device)
    }
    ).expect("No suitable GPU could be found!");
    if suitability <= 0 {panic!("No suitable GPU could be found!")}
    physical_device
}