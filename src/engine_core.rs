use std::ffi::{CStr};
use std::os::raw::{c_void, c_char};
use erupt::{vk, InstanceLoader, EntryLoader};
use erupt::cstr;

pub mod phys_device;
pub mod swapchain;

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

pub fn find_physical_device(instance: &InstanceLoader, surface: &vk::SurfaceKHR) -> vk::PhysicalDevice {
    let devices = unsafe {instance.enumerate_physical_devices(None)}.unwrap();
    if devices.len() == 0 {panic!("No devices with Vulkan support!")}

    let mut suitability = 0;
    let physical_device = devices.into_iter().max_by_key(
    |device| {
        suitability = phys_device::device_suitability(&instance, &surface, &device)
    }
    ).expect("No suitable GPU could be found!");
    if suitability <= 0 {panic!("No suitable GPU could be found!")}
    physical_device
}