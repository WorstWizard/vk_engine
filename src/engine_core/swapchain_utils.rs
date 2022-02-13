use erupt::vk;
use winit::window::Window;

pub fn choose_swap_surface_format(formats: &Vec<vk::SurfaceFormatKHR>) -> vk::SurfaceFormatKHR {
    for available_format in formats {
        if available_format.format == vk::Format::R8G8B8A8_SRGB && available_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR_KHR {
            return *available_format
        }
    }
    return formats[0];
}

pub fn choose_swap_present_mode(present_modes: &Vec<vk::PresentModeKHR>) -> vk::PresentModeKHR {
    for available_mode in present_modes {
        if *available_mode == vk::PresentModeKHR::MAILBOX_KHR {
            return *available_mode
        }
    }
    return vk::PresentModeKHR::FIFO_KHR;
}

pub fn choose_swap_extent(window: &Window, capabilities: &vk::SurfaceCapabilitiesKHR) -> vk::Extent2D {
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