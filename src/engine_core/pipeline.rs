use std::ffi::CStr;
use std::mem::size_of;
use std::os::raw::c_char;
use erupt::{vk, cstr, DeviceLoader};
use super::shaders::{Shader, ShaderType}; //Would like to avoid using super, but it's the cleanest option with the current structure
use super::Vert;

const DEFAULT_ENTRY: *const c_char = cstr!("main");

pub fn default_pipeline(
    logical_device: &DeviceLoader,
    render_pass: vk::RenderPass,
    swapchain_extent: vk::Extent2D,
    shader_modules: Vec<(vk::ShaderModule, vk::PipelineShaderStageCreateInfoBuilder)>,
    push_constants: [f32; 1],
) -> (vk::Pipeline, vk::PipelineLayout) {

    // This is all terribly bad, but works for now
    // TODO: Move it outside of this file, and fix the fucking offset being hardcoded, super dangerous if someone tries to extend it
    let binding_descriptions = [vk::VertexInputBindingDescriptionBuilder::new()
        .binding(0)
        .stride(size_of::<Vert>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX)];
    let attribute_descriptions = [vk::VertexInputAttributeDescriptionBuilder::new()
        .binding(0)
        .location(0)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(0)];

    // Vertex input settings
    let pipeline_vertex_input_state_info = vk::PipelineVertexInputStateCreateInfoBuilder::new()
        .vertex_binding_descriptions(&binding_descriptions)
        .vertex_attribute_descriptions(&attribute_descriptions);
    // Input assembly settings
    let pipeline_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfoBuilder::new()
        .topology(vk::PrimitiveTopology::TRIANGLE_STRIP)
        .primitive_restart_enable(false);
    // Viewport settings
    let viewports = [vk::ViewportBuilder::new()
        .x(0.0)
        .y(0.0)
        .width(swapchain_extent.width as f32)
        .height(swapchain_extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0)];
    let scissor_rects = [vk::Rect2DBuilder::new()
        .offset(vk::Offset2D{x: 0, y: 0})
        .extent(swapchain_extent)];
    let pipeline_viewport_state_info = vk::PipelineViewportStateCreateInfoBuilder::new()
        .viewports(&viewports)
        .scissors(&scissor_rects);
    // Rasterizer settings
    let pipeline_rasterization_state_info = vk::PipelineRasterizationStateCreateInfoBuilder::new()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::CLOCKWISE)
        .depth_bias_enable(false);
    // Multisampling settings
    let pipeline_multisample_state_info = vk::PipelineMultisampleStateCreateInfoBuilder::new()
        .sample_shading_enable(false)
        .rasterization_samples(vk::SampleCountFlagBits::_1);
    // Color blending settings
    let pipeline_color_blend_attachment_states = [vk::PipelineColorBlendAttachmentStateBuilder::new()
        .color_write_mask(
            vk::ColorComponentFlags::R |
            vk::ColorComponentFlags::G |
            vk::ColorComponentFlags::B |
            vk::ColorComponentFlags::A)
        .blend_enable(false)];
    let pipeline_color_blend_state_info = vk::PipelineColorBlendStateCreateInfoBuilder::new()
        .logic_op_enable(false)
        .attachments(&pipeline_color_blend_attachment_states);
    
    // Pipeline layout
    let push_constant_ranges = [vk::PushConstantRangeBuilder::new()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size((push_constants.len()*size_of::<f32>()) as u32)];
    
    let pipeline_layout_info = vk::PipelineLayoutCreateInfoBuilder::new()
        .push_constant_ranges(&push_constant_ranges);
    let pipeline_layout = unsafe {logical_device.create_pipeline_layout(&pipeline_layout_info, None)}.unwrap();


    let shader_stages: Vec<vk::PipelineShaderStageCreateInfoBuilder> = shader_modules.iter().map(|pair| {pair.1}).collect();
    
    let graphics_pipeline_infos = [vk::GraphicsPipelineCreateInfoBuilder::new()
        .stages(&shader_stages)
        .vertex_input_state(&pipeline_vertex_input_state_info)
        .input_assembly_state(&pipeline_input_assembly_state_info)
        .viewport_state(&pipeline_viewport_state_info)
        .rasterization_state(&pipeline_rasterization_state_info)
        .multisample_state(&pipeline_multisample_state_info)
        .color_blend_state(&pipeline_color_blend_state_info)
        .layout(pipeline_layout)
        .render_pass(render_pass)
        .subpass(0)];
    let graphics_pipeline = unsafe {logical_device.create_graphics_pipelines(vk::PipelineCache::null(), &graphics_pipeline_infos, None)}.unwrap()[0];

    //Once the graphics pipeline has been created, the SPIR-V bytecode is compiled into the pipeline itself
    //The shader modules can therefore be destroyed already
    unsafe {
        for module in shader_modules {
            logical_device.destroy_shader_module(module.0, None)
        }
    }

    (graphics_pipeline, pipeline_layout)
}





pub fn create_shader_module(logical_device: &DeviceLoader, shader: Shader) -> (vk::ShaderModule, vk::PipelineShaderStageCreateInfoBuilder) {
    let entry_point = unsafe {CStr::from_ptr(DEFAULT_ENTRY)};
    let shader_stage_flag = match shader.shader_type {
        ShaderType::Vertex => vk::ShaderStageFlagBits::VERTEX,
        ShaderType::Fragment => vk::ShaderStageFlagBits::FRAGMENT,
    };

    let decoded = &shader.data;
    let shader_module_info = vk::ShaderModuleCreateInfoBuilder::new().code(decoded);
    let shader_module = unsafe {logical_device.create_shader_module(&shader_module_info, None)}.unwrap();
    let stage_info = vk::PipelineShaderStageCreateInfoBuilder::new()
        .stage(shader_stage_flag)
        .module(shader_module)
        .name(entry_point);
    
    (shader_module, stage_info)
}

pub fn create_render_pass(logical_device: &DeviceLoader, image_format: vk::Format) -> vk::RenderPass {
    let color_attachments = [vk::AttachmentDescriptionBuilder::new()
        .format(image_format)
        .samples(vk::SampleCountFlagBits::_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
    // Subpass
    let dependencies = [vk::SubpassDependencyBuilder::new()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];
    let color_attachment_refs = [vk::AttachmentReferenceBuilder::new()
        .attachment(0) //First attachment in array -> color_attachment
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
    let subpasses = [vk::SubpassDescriptionBuilder::new()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachment_refs)];
    
    let renderpass_info = vk::RenderPassCreateInfoBuilder::new()
        .attachments(&color_attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);
    let renderpass = unsafe {logical_device.create_render_pass(&renderpass_info, None)}.expect("Failed to create renderpass!");
    renderpass
}