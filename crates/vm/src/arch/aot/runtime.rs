use std::{fs, process::Command};

use libloading::{Library, Symbol};
use tempfile::TempDir;

use super::AotHandler;

/// Runtime for executing AOT compiled code
pub struct AotRuntime {
    _temp_dir: TempDir,
    library: Library,
}

impl AotRuntime {
    /// Compile assembly to a dynamic library and load it
    pub fn compile_and_load(assembly: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Create temporary directory for build artifacts
        let temp_dir = TempDir::new()?;
        let build_dir = temp_dir.path();

        // Write assembly to file
        let asm_path = build_dir.join("aot.asm");
        fs::write(&asm_path, assembly)?;

        // Create stub C file with the external handler and sync functions
        let c_stub = r#"
#include <stdint.h>

// External handler function that will be linked
extern void openvm_aot_handler(
    const uint8_t* pre_compute,
    uint64_t* instret,
    uint32_t* pc,
    uint64_t arg,
    void* state
);

// Register sync functions that will be linked
extern void openvm_sync_registers_to_memory(
    void* state,
    const uint32_t* register_buffer
);

extern void openvm_sync_registers_from_memory(
    const void* state,
    uint32_t* register_buffer
);

// Default handler implementation (can be overridden by linking with actual implementation)
__attribute__((weak)) void openvm_aot_handler(
    const uint8_t* pre_compute,
    uint64_t* instret,
    uint32_t* pc,
    uint64_t arg,
    void* state
) {
    // Default implementation: terminate execution
    *pc = 0xFFFFFFFF;
}

// Default sync implementations (will be overridden by Rust implementations)
__attribute__((weak)) void openvm_sync_registers_to_memory(
    void* state,
    const uint32_t* register_buffer
) {
    // No-op default
}

__attribute__((weak)) void openvm_sync_registers_from_memory(
    const void* state,
    uint32_t* register_buffer
) {
    // No-op default
}
"#;

        let c_stub_path = build_dir.join("stub.c");
        fs::write(&c_stub_path, c_stub)?;

        // Compile assembly to object file
        let obj_path = build_dir.join("aot.o");
        let nasm_status = Command::new("nasm")
            .args(&["-f", "elf64", "-o"])
            .arg(&obj_path)
            .arg(&asm_path)
            .status()?;

        if !nasm_status.success() {
            return Err("NASM compilation failed".into());
        }

        // Compile C stub
        let c_obj_path = build_dir.join("stub.o");
        let gcc_status = Command::new("gcc")
            .args(&["-c", "-fPIC", "-o"])
            .arg(&c_obj_path)
            .arg(&c_stub_path)
            .status()?;

        if !gcc_status.success() {
            return Err("GCC compilation of stub failed".into());
        }

        // Link into shared library
        let lib_path = build_dir.join("libaot.so");
        let link_status = Command::new("gcc")
            .args(&["-shared", "-o"])
            .arg(&lib_path)
            .arg(&obj_path)
            .arg(&c_obj_path)
            .status()?;

        if !link_status.success() {
            return Err("Linking failed".into());
        }

        // Load the library
        let library = unsafe { Library::new(&lib_path)? };

        Ok(AotRuntime {
            _temp_dir: temp_dir,
            library,
        })
    }

    /// Get the entry point function
    pub fn get_entry_point(&self) -> Result<AotHandler, Box<dyn std::error::Error>> {
        unsafe {
            let symbol: Symbol<AotHandler> = self.library.get(b"openvm_aot_start")?;
            Ok(*symbol)
        }
    }

    /// Set a custom handler implementation
    pub fn set_handler(&self, _handler: AotHandler) -> Result<(), Box<dyn std::error::Error>> {
        // This would require more complex linking or runtime patching
        // For now, handlers must be linked at compile time
        Err("Runtime handler replacement not yet implemented".into())
    }
}

/// Builder for creating AOT runtime with custom handlers
pub struct AotRuntimeBuilder {
    assembly: String,
    handler_source: Option<String>,
}

impl AotRuntimeBuilder {
    pub fn new(assembly: String) -> Self {
        Self {
            assembly,
            handler_source: None,
        }
    }

    /// Add custom handler implementation
    pub fn with_handler_source(mut self, source: &str) -> Self {
        self.handler_source = Some(source.to_string());
        self
    }

    /// Build the runtime
    pub fn build(self) -> Result<AotRuntime, Box<dyn std::error::Error>> {
        // Create temporary directory
        let temp_dir = TempDir::new()?;
        let build_dir = temp_dir.path();

        // Write assembly
        let asm_path = build_dir.join("aot.asm");
        fs::write(&asm_path, &self.assembly)?;

        // Write handler source
        let handler_path = build_dir.join("handler.c");
        let handler_source = self.handler_source.unwrap_or_else(|| {
            // Default minimal handler
            r#"
#include <stdint.h>

void openvm_aot_handler(
    const uint8_t* pre_compute,
    uint64_t* instret,
    uint32_t* pc,
    uint64_t arg,
    void* state
) {
    // Default: terminate execution
    *pc = 0xFFFFFFFF;
}
"#
            .to_string()
        });
        fs::write(&handler_path, handler_source)?;

        // Compile assembly
        let asm_obj = build_dir.join("aot.o");
        // Use the appropriate object format for the platform
        let obj_format = if cfg!(target_os = "macos") {
            "macho64"
        } else if cfg!(target_os = "linux") {
            "elf64"
        } else {
            return Err("Unsupported platform for AOT compilation".into());
        };

        let nasm_status = Command::new("nasm")
            .args(&["-f", obj_format, "-o"])
            .arg(&asm_obj)
            .arg(&asm_path)
            .status()?;

        if !nasm_status.success() {
            return Err("NASM compilation failed".into());
        }

        // Compile handler
        let handler_obj = build_dir.join("handler.o");
        let gcc_status = Command::new("gcc")
            .args(&["-c", "-fPIC", "-o"])
            .arg(&handler_obj)
            .arg(&handler_path)
            .status()?;

        if !gcc_status.success() {
            return Err("Handler compilation failed".into());
        }

        // Link
        let lib_name = if cfg!(target_os = "macos") {
            "libaot.dylib"
        } else {
            "libaot.so"
        };
        let lib_path = build_dir.join(lib_name);
        let link_status = Command::new("gcc")
            .args(&["-shared", "-o"])
            .arg(&lib_path)
            .arg(&asm_obj)
            .arg(&handler_obj)
            .status()?;

        if !link_status.success() {
            return Err("Linking failed".into());
        }

        // Load library
        let library = unsafe { Library::new(&lib_path)? };

        Ok(AotRuntime {
            _temp_dir: temp_dir,
            library,
        })
    }
}
