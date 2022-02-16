use std::fs::File;
use std::path::Path;
use std::io::{Read, Write};
use shaderc::{Compiler, CompileOptions, ShaderKind};

pub fn load_or_compile_shader<P: AsRef<Path>>(shader_path: P, source_path: P, shader_type: ShaderKind) -> Result<Vec<u8>, &'static str>{
    let load_result = load_shader(&shader_path);
    match load_result {
        Ok(_) => return load_result,
        Err(_) => {
            return compile_shader(source_path, Some(shader_path), shader_type)
        }
    }
}

pub fn compile_shader<P: AsRef<Path>>(in_path: P, out_path: Option<P>, shader_type: ShaderKind) -> Result<Vec<u8>, &'static str> {
    if let Ok(mut file) = File::open(&in_path) {
        let file_name = in_path.as_ref().file_name().unwrap().to_str().unwrap(); //If the file loaded, this can't fail
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {return Err("Could not read shader source!")}

        // Attempt to compile code, panic on failure
        let mut compiler = Compiler::new().expect("Could not initialize SPIR-V compiler!");
        let options = CompileOptions::new().expect("Could not initialize SPIR-V compiler!");
        let bin_result = compiler.compile_into_spirv(&contents, shader_type, file_name, "main", Some(&options)).expect("Shader compilation failed!");
        let bin_slice = bin_result.as_binary_u8();

        // If saving the shader to a filez
        if out_path.is_some() {
            let mut out_file = File::create(out_path.unwrap()).unwrap();
            out_file.write_all(bin_slice).unwrap();
        }

        return Ok(bin_slice.to_vec());
    } else {return Err("Could not open shader source file!")};
}

pub fn load_shader<P: AsRef<Path>>(shader_path: P) -> Result<Vec<u8>, &'static str> {
    if let Ok(mut shader_file) = File::open(shader_path) {
        let mut contents = Vec::new();
        shader_file.read_to_end(&mut contents);
        return Ok(contents);
    }
    Err("Could not load shader!")
}