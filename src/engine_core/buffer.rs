use ash::{vk, Device, Instance};
use std::ffi::c_void;
use std::ops::Deref;
use std::rc::Rc;

pub struct ManagedBuffer {
    pub logical_device: Rc<Device>,
    pub memory_size: vk::DeviceSize,
    pub buffer_memory: Option<vk::DeviceMemory>,
    pub buffer: vk::Buffer,
    pub memory_ptr: Option<*mut c_void>,
}
impl ManagedBuffer {
    /// Maps whole of the allocated buffer memory, returns pointer to the data.
    /// Invalid if memory is not visible to the host device (unsure what happens if not).
    /// Panics if there's no memory to map
    pub fn map_buffer_memory(&mut self) {
        if let Some(memory) = self.buffer_memory {
            if self.memory_ptr.is_some() {
                panic!("Attempt to re-map buffer memory!")
            }
            self.memory_ptr = Some(map_buffer_memory(&self.logical_device, memory))
        } else {
            panic!("Attempt to map unallocated/unbound buffer memory!");
        }
    }

    /// Unmaps buffer memory (unsure what happens if it isn't mapped. no-op?)
    /// Panics if there's no memory to unmap (does it matter?)
    pub fn unmap_buffer_memory(&mut self) {
        if self.memory_ptr.is_some() {
            unsafe {
                self.logical_device
                    .unmap_memory(self.buffer_memory.unwrap())
            };
        } else {
            panic!("Attempt to unmap unmapped buffer memory!");
        }
    }
}
impl Deref for ManagedBuffer {
    type Target = vk::Buffer;
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
impl Drop for ManagedBuffer {
    fn drop(&mut self) {
        unsafe {
            if self.memory_ptr.is_some() {
                self.unmap_buffer_memory();
            }
            if let Some(memory) = self.buffer_memory {
                self.logical_device.free_memory(memory, None);
            }
            self.logical_device.destroy_buffer(self.buffer, None);
        }
    }
}

pub fn map_buffer_memory(logical_device: &Device, buffer_memory: vk::DeviceMemory) -> *mut c_void {
    unsafe {
        logical_device.map_memory(
            buffer_memory,
            0,
            vk::WHOLE_SIZE,
            vk::MemoryMapFlags::empty(),
        )
    }
    .unwrap()
}

/// Refer to https://doc.rust-lang.org/reference/type-layout.html for info on data layout.
pub fn create_buffer(
    logical_device: &Device,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
) -> vk::Buffer {
    let buffer_info = vk::BufferCreateInfo::builder()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    unsafe { logical_device.create_buffer(&buffer_info, None) }.unwrap()
}

pub fn allocate_and_bind_buffer(
    instance: &Instance,
    physical_device: &vk::PhysicalDevice,
    logical_device: &Device,
    buffer: vk::Buffer,
    memory_properties: vk::MemoryPropertyFlags,
) -> vk::DeviceMemory {
    let memory_requirements = unsafe { logical_device.get_buffer_memory_requirements(buffer) };
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
    let buffer_memory = unsafe { logical_device.allocate_memory(&mem_alloc_info, None) }.unwrap();
    unsafe { logical_device.bind_buffer_memory(buffer, buffer_memory, 0) }.unwrap();

    buffer_memory
}
