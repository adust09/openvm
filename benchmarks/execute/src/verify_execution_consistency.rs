use eyre::Result;
use openvm_benchmarks_utils::{get_elf_path, get_programs_dir, read_elf_file};
use openvm_circuit::{
    arch::{
        execution_mode::metered::{MeteredCtx, Segment},
        instructions::exe::VmExe,
        PreflightExecutionOutput, VirtualMachine, *,
    },
    system::memory::online::{GuestMemory, LinearMemory},
};
use openvm_sdk::config::{SdkVmConfig, SdkVmCpuBuilder};
use openvm_stark_sdk::{
    config::{baby_bear_poseidon2::BabyBearPoseidon2Engine, FriParameters},
    engine::StarkFriEngine,
    p3_baby_bear::BabyBear,
};
use openvm_transpiler::FromElf;
use tracing_subscriber::{fmt, EnvFilter};

static AVAILABLE_PROGRAMS: &[&str] = &[
    "fibonacci_recursive",
    "fibonacci_iterative",
    "quicksort",
    "bubblesort",
    // "factorial_iterative_u256",
    "revm_snailtracer",
    "keccak256",
    // "keccak256_iter",
    "sha256",
    // "sha256_iter",
    "revm_transfer",
    "pairing",
    "kitchen-sink",
];

fn load_program_executable(program: &str) -> Result<(VmExe<BabyBear>, SdkVmConfig)> {
    let program_dir = get_programs_dir().join(program);
    let elf_path = get_elf_path(&program_dir);
    let elf = read_elf_file(&elf_path)?;

    // Load config from TOML file for each program
    let config_path = program_dir.join("openvm.toml");
    let config_str = std::fs::read_to_string(&config_path)?;
    let vm_config = SdkVmConfig::from_toml(&config_str)?.app_vm_config;

    let exe = VmExe::from_elf(elf, vm_config.transpiler())?;
    Ok((exe, vm_config))
}

fn metering_setup(config: &SdkVmConfig) -> (MeteredCtx, Vec<usize>) {
    let engine = BabyBearPoseidon2Engine::new(FriParameters::standard_fast());
    let (vm, _) = VirtualMachine::new_with_keygen(engine, SdkVmCpuBuilder, config.clone()).unwrap();
    let ctx = vm.build_metered_ctx();
    let executor_idx_to_air_idx = vm.executor_idx_to_air_idx();
    (ctx, executor_idx_to_air_idx)
}

fn executor(config: SdkVmConfig) -> VmExecutor<BabyBear, SdkVmConfig> {
    VmExecutor::<BabyBear, _>::new(config).unwrap()
}

fn create_vm(config: SdkVmConfig) -> VirtualMachine<BabyBearPoseidon2Engine, SdkVmCpuBuilder> {
    let engine = BabyBearPoseidon2Engine::new(FriParameters::standard_fast());
    let (vm, _) = VirtualMachine::new_with_keygen(engine, SdkVmCpuBuilder, config).unwrap();
    vm
}

#[derive(Debug)]
struct ExecutionResult {
    instret: u64,
    pc: u32,
    memory: GuestMemory,
}

impl PartialEq for ExecutionResult {
    fn eq(&self, other: &Self) -> bool {
        self.instret == other.instret && self.pc == other.pc && self.memory_equal(&other.memory)
    }
}

impl ExecutionResult {
    fn memory_equal(&self, other: &GuestMemory) -> bool {
        for (mem1, mem2) in self.memory.memory.mem.iter().zip(other.memory.mem.iter()) {
            if mem1.as_slice() != mem2.as_slice() {
                return false;
            }
        }
        true
    }

    fn detailed_compare(&self, other: &Self, self_name: &str, other_name: &str) {
        if self.instret != other.instret {
            tracing::error!(
                "  Instruction count: {} = {}, {} = {}",
                self_name,
                self.instret,
                other_name,
                other.instret
            );
        }

        if self.pc != other.pc {
            tracing::error!(
                "  Final PC: {} = {}, {} = {}",
                self_name,
                self.pc,
                other_name,
                other.pc
            );
        }

        if !self.memory_equal(&other.memory) {
            self.detailed_memory_compare(&other.memory, self_name, other_name);
        }
    }

    fn detailed_memory_compare(&self, other: &GuestMemory, self_name: &str, other_name: &str) {
        for (addr_space_idx, (mem1, mem2)) in self
            .memory
            .memory
            .mem
            .iter()
            .zip(other.memory.mem.iter())
            .enumerate()
        {
            let mem1_bytes = mem1.as_slice();
            let mem2_bytes = mem2.as_slice();

            if mem1_bytes.len() != mem2_bytes.len() {
                tracing::error!(
                    "  Address space {} size: {} = {}, {} = {}",
                    addr_space_idx,
                    self_name,
                    mem1_bytes.len(),
                    other_name,
                    mem2_bytes.len()
                );
                continue;
            }

            // Find all differences
            let mut diff_count = 0;
            for (offset, (&byte1, &byte2)) in mem1_bytes.iter().zip(mem2_bytes.iter()).enumerate() {
                if byte1 != byte2 {
                    tracing::error!(
                        "  Address space {} diff at offset {}: {} = 0x{:02x}, {} = 0x{:02x}",
                        addr_space_idx,
                        offset,
                        self_name,
                        byte1,
                        other_name,
                        byte2
                    );
                    diff_count += 1;
                }
            }

            if diff_count > 0 {
                tracing::error!(
                    "  Address space {} has {} total differences",
                    addr_space_idx,
                    diff_count
                );
            }
        }
    }
}

fn verify_results_equal(results: &[(&str, &ExecutionResult)]) -> bool {
    let (first_name, first_result) = results[0];
    let mut all_equal = true;

    for (name, result) in &results[1..] {
        if result != &first_result {
            tracing::error!("{} execution differs from {}", name, first_name);
            first_result.detailed_compare(result, first_name, name);
            all_equal = false;
        }
    }

    if all_equal {
        tracing::info!("All execution modes produce identical results");
    }
    all_equal
}

fn run_basic_execution(
    exe: &VmExe<BabyBear>,
    config: SdkVmConfig,
    input: Vec<Vec<BabyBear>>,
) -> Result<ExecutionResult> {
    tracing::debug!("Running basic execution");
    let executor = executor(config);
    let interpreter = executor.instance(exe)?;
    let state = interpreter.execute(input, None)?;

    Ok(ExecutionResult {
        instret: state.instret,
        pc: state.pc,
        memory: state.memory,
    })
}

fn run_metered_execution(
    exe: &VmExe<BabyBear>,
    config: SdkVmConfig,
    input: Vec<Vec<BabyBear>>,
) -> Result<(Vec<Segment>, ExecutionResult)> {
    tracing::debug!("Running metered execution");
    let (ctx, executor_idx_to_air_idx) = metering_setup(&config);
    let executor = executor(config);
    let interpreter = executor.metered_instance(exe, &executor_idx_to_air_idx)?;

    let (segments, state) = interpreter.execute_metered(input, ctx.clone())?;

    Ok((
        segments,
        ExecutionResult {
            instret: state.instret,
            pc: state.pc,
            memory: state.memory,
        },
    ))
}

fn run_preflight_execution(
    exe: &VmExe<BabyBear>,
    config: SdkVmConfig,
    input: Vec<Vec<BabyBear>>,
    segments: &[Segment],
) -> Result<ExecutionResult> {
    tracing::debug!("Running preflight execution");
    let vm = create_vm(config);

    // Use the first segment's trace heights (assuming single segment for these programs)
    let trace_heights = if !segments.is_empty() {
        &segments[0].trace_heights
    } else {
        // Fallback to reasonable defaults if no segments
        return Err(eyre::eyre!("No segments available for preflight execution"));
    };

    // Create initial state using VM's method
    let initial_state = vm.create_initial_state(exe, input);

    // Run preflight execution
    let preflight_output: PreflightExecutionOutput<BabyBear, _> =
        vm.execute_preflight(exe, initial_state, None, trace_heights)?;

    Ok(ExecutionResult {
        instret: preflight_output.to_state.instret,
        pc: preflight_output.to_state.pc,
        memory: preflight_output.to_state.memory,
    })
}

fn main() -> Result<()> {
    // Set up logging
    fmt::fmt()
        .with_env_filter(EnvFilter::new("verify_execution_consistency=info"))
        .init();

    tracing::info!("Starting execution consistency verification");

    for program in AVAILABLE_PROGRAMS {
        tracing::info!("Testing program: {}", program);

        let (exe, vm_config) = load_program_executable(program)?;
        let input = vec![];

        // 1. Run basic execution
        let basic_result = run_basic_execution(&exe, vm_config.clone(), input.clone())?;
        tracing::info!(
            "Basic execute completed: {} instructions",
            basic_result.instret
        );

        // 2. Run metered execution
        let (segments, metered_result) =
            run_metered_execution(&exe, vm_config.clone(), input.clone())?;
        let total_metered_instret: u64 = segments.iter().map(|s| s.num_insns).sum();
        tracing::info!(
            "Metered execute completed: {} instructions across {} segments",
            total_metered_instret,
            segments.len()
        );

        // 3. Run preflight execution
        let preflight_result = run_preflight_execution(&exe, vm_config, input, &segments)?;
        tracing::info!(
            "Preflight execute completed: {} instructions",
            preflight_result.instret
        );

        // Verify all execution modes produce identical results
        let results = [
            ("basic", &basic_result),
            ("metered", &metered_result),
            ("preflight", &preflight_result),
        ];

        verify_results_equal(&results);

        println!();
    }

    Ok(())
}
