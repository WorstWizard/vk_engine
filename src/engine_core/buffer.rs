use erupt::{vk, InstanceLoader, DeviceLoader};
use std::ffi::c_void;

/// Refer to https://doc.rust-lang.org/reference/type-layout.html for info on data layout.
pub fn create_buffer(logical_device: &DeviceLoader, size: vk::DeviceSize, usage: vk::BufferUsageFlags) -> vk::Buffer {
    let buffer_info = vk::BufferCreateInfoBuilder::new()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    unsafe{ logical_device.create_buffer(&buffer_info, None) }.unwrap()
}

pub fn allocate_and_bind_buffer(instance: &InstanceLoader, physical_device: &vk::PhysicalDevice, logical_device: &DeviceLoader, buffer: vk::Buffer, memory_properties: vk::MemoryPropertyFlags) -> vk::DeviceMemory {
    let memory_requirements = unsafe{ logical_device.get_buffer_memory_requirements(buffer) };
    fn find_memory_type(instance: &InstanceLoader, physical_device: vk::PhysicalDevice, type_filter: u32, properties: vk::MemoryPropertyFlags) -> Result<(u32, vk::MemoryType), &str> {
        let memory_properties = unsafe{ instance.get_physical_device_memory_properties(physical_device) };
        for (i, mem_type) in memory_properties.memory_types.into_iter().enumerate() {
            if (type_filter & (1 << i)) != 0 && (mem_type.property_flags.contains(properties)) {
                return Ok((i as u32, mem_type));
            }
        }
        return Err("No suitable memory type found!");
    }

    let mem_alloc_info = vk::MemoryAllocateInfoBuilder::new()
        .allocation_size(memory_requirements.size)
        .memory_type_index(
            find_memory_type(
                &instance,
                *physical_device,
                memory_requirements.memory_type_bits,
                memory_properties
            ).unwrap().0
        );
    let buffer_memory = unsafe {logical_device.allocate_memory(&mem_alloc_info, None)}.unwrap();
    unsafe {logical_device.bind_buffer_memory(buffer, buffer_memory, 0)}.unwrap();
    
    buffer_memory
}

pub fn map_buffer_memory(logical_device: &DeviceLoader, buffer_memory: vk::DeviceMemory) -> *mut c_void {
    unsafe {logical_device.map_memory(buffer_memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())}.unwrap()
}