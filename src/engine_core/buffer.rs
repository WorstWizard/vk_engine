use erupt::{vk, InstanceLoader, DeviceLoader};
use std::mem::size_of;
use std::ffi::c_void;

#[repr(C)]
pub struct Vert(pub f32, pub f32);

pub fn create_vertex_buffer(logical_device: &DeviceLoader, size: vk::DeviceSize) -> vk::Buffer {
    let buffer_info = vk::BufferCreateInfoBuilder::new()
        .size(size)
        .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    unsafe{ logical_device.create_buffer(&buffer_info, None) }.unwrap()
}

pub fn allocate_and_bind_buffer(instance: &InstanceLoader, physical_device: vk::PhysicalDevice, logical_device: &DeviceLoader, buffer: vk::Buffer) -> (*mut c_void, vk::DeviceMemory) {
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
                physical_device,
                memory_requirements.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
            ).unwrap().0
        );
    let buffer_memory = unsafe {logical_device.allocate_memory(&mem_alloc_info, None)}.unwrap();
    unsafe {logical_device.bind_buffer_memory(buffer, buffer_memory, 0)}.unwrap();
    let buffer_pointer = unsafe {logical_device.map_memory(buffer_memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())}.unwrap();
    (buffer_pointer, buffer_memory)
}

pub unsafe fn write_to_buffer<T>(buffer_pointer: *mut c_void, data: T) {
    let dat_ptr = buffer_pointer as *mut T;
    std::ptr::write(dat_ptr, data);
}