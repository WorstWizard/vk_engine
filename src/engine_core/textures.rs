use std::ffi::c_void;
use std::rc::Rc;

use ash::{vk, Device, Instance};

pub struct ManagedImage {
    pub logical_device: Rc<Device>,
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    pub image_memory: Option<vk::DeviceMemory>,
    pub memory_ptr: Option<*mut c_void>,
}
impl ManagedImage {
    /// Maps whole of the allocated image memory, returns pointer to the data.
    /// Invalid if memory is not visible to the host device (unsure what happens if not).
    /// Panics if there's no memory to map
    pub fn map_image_memory(&mut self) {
        if let Some(memory) = self.image_memory {
            if self.memory_ptr.is_some() {
                panic!("Attempt to re-map image memory!")
            }
            self.memory_ptr = Some(map_image_memory(&self.logical_device, memory))
        } else {
            panic!("Attempt to map unallocated/unbound image memory!");
        }
    }
    /// Unmaps image memory (unsure what happens if it isn't mapped. no-op?)
    /// Panics if there's no memory to unmap (does it matter?)
    pub fn unmap_image_memory(&mut self) {
        if self.memory_ptr.is_some() {
            unsafe { self.logical_device.unmap_memory(self.image_memory.unwrap()) };
        } else {
            panic!("Attempt to unmap unmapped image memory!");
        }
    }
}

impl Drop for ManagedImage {
    fn drop(&mut self) {
        unsafe {
            if self.memory_ptr.is_some() {
                self.unmap_image_memory();
            }
            if let Some(memory) = self.image_memory {
                self.logical_device.free_memory(memory, None);
            }
            self.logical_device.destroy_image_view(self.image_view, None);
            self.logical_device.destroy_image(self.image, None);
        }
    }
}

pub fn map_image_memory(logical_device: &Device, image_memory: vk::DeviceMemory) -> *mut c_void {
    unsafe {
        logical_device.map_memory(image_memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
    }
    .unwrap()
}

pub fn allocate_and_bind_image(
    instance: &Instance,
    physical_device: &vk::PhysicalDevice,
    logical_device: &Device,
    image: vk::Image,
    memory_properties: vk::MemoryPropertyFlags,
) -> vk::DeviceMemory {
    let memory_requirements = unsafe { logical_device.get_image_memory_requirements(image) };
    fn find_memory_type(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<(u32, vk::MemoryType), &str> {
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        for (i, mem_type) in memory_properties.memory_types.into_iter().enumerate() {
            if (type_filter & (1 << i)) != 0 && (mem_type.property_flags.contains(properties)) {
                return Ok((i as u32, mem_type));
            }
        }
        Err("No suitable memory type found!")
    }

    let mem_alloc_info = vk::MemoryAllocateInfo::builder()
        .allocation_size(memory_requirements.size)
        .memory_type_index(
            find_memory_type(
                instance,
                *physical_device,
                memory_requirements.memory_type_bits,
                memory_properties,
            )
            .unwrap()
            .0,
        );
    // May hit allocation limit if too many separate allocations are performed; use some allocator to do many objects with few buffers
    let image_memory = unsafe { logical_device.allocate_memory(&mem_alloc_info, None) }.unwrap();
    unsafe { logical_device.bind_image_memory(image, image_memory, 0) }.unwrap();

    image_memory
}

pub fn create_texture_image(logical_device: &Device, format: vk::Format, dimensions: (u32, u32)) -> vk::Image {
    let img_create_info = vk::ImageCreateInfo::builder()
        .image_type(vk::ImageType::TYPE_2D)
        .extent(vk::Extent3D {
            width: dimensions.0,
            height: dimensions.1,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .format(format)
        .tiling(vk::ImageTiling::OPTIMAL)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .samples(vk::SampleCountFlags::TYPE_1);

    let image = unsafe { logical_device.create_image(&img_create_info, None) }.unwrap();

    image
}

pub fn create_texture_image_view(logical_device: &Device, image: vk::Image, format: vk::Format) -> vk::ImageView {
    let image_view = vk::ImageViewCreateInfo::builder()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(
            *vk::ImageSubresourceRange::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
        );
    unsafe { logical_device.create_image_view(&image_view, None) }
        .expect("Failed to create image view!")
}