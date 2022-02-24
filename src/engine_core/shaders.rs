use std::fs::File;
use std::path::Path;
use std::io::{Read, Write};
use shaderc::{Compiler, CompileOptions, ShaderKind};

pub struct Shader {
    pub data: Vec<u32>,
    pub shader_type: ShaderType,
}

#[derive(Clone, Copy)]
pub enum ShaderType {
    Vertex,
    Fragment,
}
impl From<ShaderType> for ShaderKind {
    fn from(shader_type: ShaderType) -> ShaderKind{
        match shader_type {
            ShaderType::Vertex => ShaderKind::Vertex,
            ShaderType::Fragment => ShaderKind::Fragment,
        }
    }
}

pub fn load_or_compile_shader<P: AsRef<Path>>(shader_path: P, source_path: P, shader_type: ShaderType) -> Result<Shader, &'static str>{
    let load_result = load_shader(&shader_path, shader_type);
    match load_result {
        Ok(_) => return load_result,
        Err(_) => {
            return compile_shader(source_path, Some(shader_path), shader_type)
        }
    }
}

pub fn compile_shader<P: AsRef<Path>>(in_path: P, out_path: Option<P>, shader_type: ShaderType) -> Result<Shader, &'static str> {
    if let Ok(mut file) = File::open(&in_path) {
        let file_name = in_path.as_ref().file_name().unwrap().to_str().unwrap(); //If the file loaded, this can't fail
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {return Err("Could not read shader source!")}

        // Attempt to compile code, panic on failure
        let mut compiler = Compiler::new().expect("Could not initialize SPIR-V compiler!");
        let options = CompileOptions::new().expect("Could not initialize SPIR-V compiler!");
        let bin_result = compiler.compile_into_spirv(&contents, ShaderKind::from(shader_type), file_name, "main", Some(&options)).expect("Shader compilation failed!");
        let bin_slice = bin_result.as_binary_u8();

        // If saving the shader to a file
        if out_path.is_some() {
            let mut out_file = File::create(out_path.unwrap()).unwrap();
            out_file.write_all(bin_slice).unwrap();
        }

        return Ok(Shader{
            data: bin_result.as_binary().to_vec(),
            shader_type: shader_type,
        })
    }
    Err("Could not open shader source file!")
}

pub fn load_shader<P: AsRef<Path>>(shader_path: P, shader_type: ShaderType) -> Result<Shader, &'static str> {
    if let Ok(mut shader_file) = File::open(shader_path) {
        let mut contents = Vec::new();
        shader_file.read_to_end(&mut contents).unwrap();
        if let Ok(decoded_spv) = erupt::utils::decode_spv(&contents) {
            return Ok(Shader{
                data: decoded_spv,
                shader_type: shader_type,
            })
        }
    }
    Err("Could not load shader!")
}