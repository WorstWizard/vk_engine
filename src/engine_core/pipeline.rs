use crate::shaders::{Shader, ShaderType};
use ash::{
    vk::{self, DescriptorSetLayoutBinding},
    Device,
};
use cstr::cstr;
use glam::*;
use std::ffi::CStr;
use std::mem::size_of;
use std::os::raw::c_char;

const DEFAULT_ENTRY: *const c_char = cstr!("main").as_ptr();

pub fn default_pipeline(
    logical_device: &Device,
    render_pass: vk::RenderPass,
    swapchain_extent: vk::Extent2D,
    shaders: (Shader, Shader),
    push_constants: [f32; 1],
) -> (vk::Pipeline, vk::PipelineLayout, vk::DescriptorSetLayout) {
    // This is all terribly bad, but works for now
    // TODO: Move it outside of this file, and fix the fucking offset being hardcoded, super dangerous if someone tries to extend it
    let binding_descriptions = [*vk::VertexInputBindingDescription::builder()
        .binding(0)
        .stride(size_of::<Vec2>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX)];
    let attribute_descriptions = [*vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(0)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(0)];

    // Vertex input settings
    let pipeline_vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&binding_descriptions)
        .vertex_attribute_descriptions(&attribute_descriptions);
    // Input assembly settings
    let pipeline_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false);
    // Viewport settings
    let viewports = [*vk::Viewport::builder()
        .x(0.0)
        .y(0.0)
        .width(swapchain_extent.width as f32)
        .height(swapchain_extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0)];
    let scissor_rects = [*vk::Rect2D::builder()
        .offset(vk::Offset2D { x: 0, y: 0 })
        .extent(swapchain_extent)];
    let pipeline_viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
        .viewports(&viewports)
        .scissors(&scissor_rects);
    // Rasterizer settings
    let pipeline_rasterization_state_info = vk::PipelineRasterizationStateCreateInfo::builder()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::CLOCKWISE)
        .depth_bias_enable(false);
    // Multisampling settings
    let pipeline_multisample_state_info = vk::PipelineMultisampleStateCreateInfo::builder()
        .sample_shading_enable(false)
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    // Color blending settings
    let pipeline_color_blend_attachment_states =
        [*vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(
                vk::ColorComponentFlags::R
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::A,
            )
            .blend_enable(false)];
    let pipeline_color_blend_state_info = vk::PipelineColorBlendStateCreateInfo::builder()
        .logic_op_enable(false)
        .attachments(&pipeline_color_blend_attachment_states);

    // Descriptor set layout
    let descriptor_set_binding = [*DescriptorSetLayoutBinding::builder()
        .binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::VERTEX)];
    let descriptor_set_layout_info =
        vk::DescriptorSetLayoutCreateInfo::builder().bindings(&descriptor_set_binding);
    let descriptor_set_layout =
        [
            unsafe {
                logical_device.create_descriptor_set_layout(&descriptor_set_layout_info, None)
            }
            .unwrap(),
        ];

    // Pipeline layout
    let push_constant_ranges = [*vk::PushConstantRange::builder()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size((push_constants.len() * size_of::<f32>()) as u32)];

    let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
        .set_layouts(&descriptor_set_layout)
        .push_constant_ranges(&push_constant_ranges);
    let pipeline_layout =
        unsafe { logical_device.create_pipeline_layout(&pipeline_layout_info, None) }.unwrap();

    let shader_modules = [
        create_shader_module(logical_device, shaders.0),
        create_shader_module(logical_device, shaders.1),
    ];

    let shader_stages: Vec<vk::PipelineShaderStageCreateInfo> =
        shader_modules.iter().map(|pair| *pair.1).collect();

    let graphics_pipeline_infos = [*vk::GraphicsPipelineCreateInfo::builder()
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
    let graphics_pipeline = unsafe {
        logical_device.create_graphics_pipelines(
            vk::PipelineCache::null(),
            &graphics_pipeline_infos,
            None,
        )
    }
    .unwrap()[0];

    //Once the graphics pipeline has been created, the SPIR-V bytecode is compiled into the pipeline itself
    //The shader modules can therefore already be destroyed
    unsafe {
        for module in shader_modules {
            logical_device.destroy_shader_module(module.0, None)
        }
    }

    (graphics_pipeline, pipeline_layout, descriptor_set_layout[0])
}

pub fn default_render_pass(logical_device: &Device, image_format: vk::Format) -> vk::RenderPass {
    let color_attachments = [*vk::AttachmentDescription::builder()
        .format(image_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
    // Subpass
    let dependencies = [*vk::SubpassDependency::builder()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];
    let color_attachment_refs = [*vk::AttachmentReference::builder()
        .attachment(0) //First attachment in array -> color_attachment
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
    let subpasses = [*vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachment_refs)];

    let renderpass_info = vk::RenderPassCreateInfo::builder()
        .attachments(&color_attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);

    unsafe { logical_device.create_render_pass(&renderpass_info, None) }
        .expect("Failed to create renderpass!")
}

fn create_shader_module(
    logical_device: &Device,
    shader: Shader,
) -> (vk::ShaderModule, vk::PipelineShaderStageCreateInfoBuilder) {
    let entry_point = unsafe { CStr::from_ptr(DEFAULT_ENTRY) };
    let shader_stage_flag = match shader.shader_type {
        ShaderType::Vertex => vk::ShaderStageFlags::VERTEX,
        ShaderType::Fragment => vk::ShaderStageFlags::FRAGMENT,
    };

    let decoded = &shader.data;
    let shader_module_info = vk::ShaderModuleCreateInfo::builder().code(decoded);
    let shader_module =
        unsafe { logical_device.create_shader_module(&shader_module_info, None) }.unwrap();
    let stage_info = vk::PipelineShaderStageCreateInfo::builder()
        .stage(shader_stage_flag)
        .module(shader_module)
        .name(entry_point);

    (shader_module, stage_info)
}
