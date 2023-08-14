use ash::vk;
use winit::window::Window;

// Surface format details how images are represented in memory
pub fn choose_swap_surface_format(formats: &Vec<vk::SurfaceFormatKHR>) -> vk::SurfaceFormatKHR {
    for available_format in formats {
        // If preferred format available, return it
        if available_format.format == vk::Format::R8G8B8A8_SRGB
            && available_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        {
            return *available_format;
        }
    }
    // Otherwise use the first in list (usually good)
    formats[0]
}
// How are images presented to the surface from the swapchain. IMMEDIATE_KHR to turn any vertical sync off, FIFO_KHR is "normal" vsync
// MAILBOX_KHR is preferred option for vsync with low latency; images at the back of the queue are replaced
pub fn choose_swap_present_mode(
    present_modes: &Vec<vk::PresentModeKHR>,
    preferred_mode: vk::PresentModeKHR,
) -> vk::PresentModeKHR {
    for available_mode in present_modes {
        if *available_mode == preferred_mode {
            return *available_mode;
        }
    }
    vk::PresentModeKHR::FIFO
}

pub fn choose_swap_extent(
    window: &Window,
    capabilities: &vk::SurfaceCapabilitiesKHR,
) -> vk::Extent2D {
    //If width/height of current extent is u32::MAX, the window manager allows selecting an extent different from the window resolution
    if capabilities.current_extent.width != u32::MAX {
        //Extent is specified already, must use it
        capabilities.current_extent
    } else {
        let window_size = window.inner_size();
        let mut actual_extent = vk::Extent2D {
            width: window_size.width,
            height: window_size.height,
        };
        actual_extent.width = actual_extent.width.clamp(
            capabilities.min_image_extent.width,
            capabilities.max_image_extent.width,
        );
        actual_extent.height = actual_extent.height.clamp(
            capabilities.min_image_extent.height,
            capabilities.max_image_extent.height,
        );
        actual_extent
    }
}
