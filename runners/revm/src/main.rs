use clap::Parser;
use revm::{
    interpreter::{
        opcode::make_instruction_table,
        primitives::{address, hex, Bytes, Env, TransactTo},
        Contract, DummyHost, Interpreter, SharedMemory,
    },
    primitives::{CancunSpec, ExecutionResult, Output, ResultAndState, SpecId},
    Evm,
};
use revm_jit::llvm::{with_llvm_context, Context};
use std::{fs, path::PathBuf, time::Instant};

/// Revolutionary EVM (revm) runner interface
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Whether to use JIT
    #[arg(long)]
    jit: bool,

    /// Path to the hex contract code to deploy and run
    #[arg(long)]
    contract_code_path: PathBuf,

    /// Hex of calldata to use when calling the contract
    #[arg(long)]
    calldata: String,

    /// Number of times to run the benchmark
    #[arg(short, long, default_value_t = 1)]
    num_runs: u8,
}

fn main() {
    with_llvm_context(main_);
}

fn main_(cx: &Context) {
    let args = Args::parse();

    let creation_code_hex =
        fs::read_to_string(args.contract_code_path).expect("failed to read code path");
    let creation_code: Bytes =
        hex::decode(creation_code_hex.trim()).expect("could not hex decode contract code").into();
    let calldata: Bytes =
        hex::decode(args.calldata.trim()).expect("could not hex decode calldata").into();

    let caller = address!("1000000000000000000000000000000000000001");

    // Set up and run the EVM to create the contract.
    let mut evm = Evm::builder()
        .with_empty_db()
        .modify_tx_env(|tx| {
            tx.caller = caller;
            tx.transact_to = TransactTo::create();
            tx.data = creation_code;
        })
        .build();
    let ResultAndState { result, state } = evm.transact().expect("EVM failed");
    let ExecutionResult::Success { output, .. } = result else {
        panic!("failed executing bytecode: {result:#?}");
    };
    let Output::Create(_, Some(created_address)) = output else {
        panic!("failed creating contract: {output:#?}");
    };

    // Run the created bytecode with just the interpreter.
    let created_bytecode = state[&created_address].info.code.as_ref().expect("failed creation");

    let mut run_env = Env::default();
    run_env.tx.caller = caller;
    run_env.tx.transact_to = TransactTo::call(created_address);
    run_env.tx.data = calldata;

    let contract = Contract::new_env(&run_env, created_bytecode.clone(), None);
    let mut host = DummyHost::new(run_env);
    let table = &make_instruction_table::<_, CancunSpec>();

    let mut compiler;
    let jit_function = if args.jit {
        let opt_level = revm_jit::OptimizationLevel::Aggressive;
        let backend = revm_jit::new_llvm_backend(cx, false, opt_level).unwrap();
        compiler = revm_jit::EvmCompiler::new(backend);
        let bytecode = created_bytecode.original_byte_slice();
        Some(compiler.jit(None, bytecode, SpecId::CANCUN).unwrap())
    } else {
        None
    };

    for _ in 0..args.num_runs {
        let mut interpreter = Interpreter::new(contract.clone(), u64::MAX, false);

        let timer = Instant::now();
        let action = if let Some(f) = jit_function {
            unsafe { f.call_with_interpreter(&mut interpreter, &mut host) }
        } else {
            interpreter.run(SharedMemory::new(), table, &mut host)
        };
        let dur = timer.elapsed();

        assert!(
            interpreter.instruction_result.is_ok(),
            "interpreter failed with {:?}",
            interpreter.instruction_result
        );
        assert!(action.is_return(), "unexpected exit action: {action:?}");

        host.clear();

        println!("{}", dur.as_secs_f64() * 1000.0)
    }
}
