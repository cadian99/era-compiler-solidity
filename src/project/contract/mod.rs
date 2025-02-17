//!
//! The contract data.
//!

pub mod ir;
pub mod metadata;

use std::collections::HashSet;

use serde::Deserialize;
use serde::Serialize;
use sha3::Digest;

use compiler_llvm_context::WriteLLVM;

use crate::build::contract::Contract as ContractBuild;
use crate::project::Project;

use self::ir::IR;
use self::metadata::Metadata;

///
/// The contract data.
///
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Contract {
    /// The absolute file path.
    pub path: String,
    /// The IR source code data.
    pub ir: IR,
    /// The metadata JSON.
    pub metadata_json: serde_json::Value,
}

impl Contract {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        path: String,
        source_hash: [u8; compiler_common::BYTE_LENGTH_FIELD],
        source_version: semver::Version,
        ir: IR,
        metadata_json: Option<serde_json::Value>,
    ) -> Self {
        Self {
            path,
            ir,
            metadata_json: metadata_json.unwrap_or_else(|| {
                serde_json::json!({
                    "source_hash": hex::encode(source_hash.as_slice()),
                    "source_version": source_version.to_string(),
                })
            }),
        }
    }

    ///
    /// Returns the contract identifier, which is:
    /// - the Yul object identifier for Yul
    /// - the full contract path for EVM legacy assembly
    /// - the module name for LLVM IR
    ///
    pub fn identifier(&self) -> &str {
        match self.ir {
            IR::Yul(ref yul) => yul.object.identifier.as_str(),
            IR::EVMLA(ref evm) => evm.assembly.full_path(),
            IR::LLVMIR(ref llvm_ir) => llvm_ir.path.as_str(),
            IR::ZKASM(ref zkasm) => zkasm.path.as_str(),
        }
    }

    ///
    /// Extract factory dependencies.
    ///
    pub fn drain_factory_dependencies(&mut self) -> HashSet<String> {
        match self.ir {
            IR::Yul(ref mut yul) => yul.object.factory_dependencies.drain().collect(),
            IR::EVMLA(ref mut evm) => evm.assembly.factory_dependencies.drain().collect(),
            IR::LLVMIR(_) => HashSet::new(),
            IR::ZKASM(_) => HashSet::new(),
        }
    }

    ///
    /// Compiles the specified contract, setting its build artifacts.
    ///
    pub fn compile(
        mut self,
        project: Project,
        optimizer_settings: compiler_llvm_context::OptimizerSettings,
        is_system_mode: bool,
        include_metadata_hash: bool,
        debug_config: Option<compiler_llvm_context::DebugConfig>,
    ) -> anyhow::Result<ContractBuild> {
        let llvm = inkwell::context::Context::create();
        let optimizer = compiler_llvm_context::Optimizer::new(optimizer_settings);

        let metadata = Metadata::new(
            self.metadata_json.take(),
            semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("Always valid"),
            optimizer.settings().to_owned(),
        );
        let metadata_json = serde_json::to_value(&metadata).expect("Always valid");
        let metadata_hash: Option<[u8; compiler_common::BYTE_LENGTH_FIELD]> =
            if include_metadata_hash {
                let metadata_string = serde_json::to_string(&metadata).expect("Always valid");
                Some(sha3::Keccak256::digest(metadata_string.as_bytes()).into())
            } else {
                None
            };

        let version = project.version.clone();
        let identifier = self.identifier().to_owned();

        let module = match self.ir {
            IR::LLVMIR(ref llvm_ir) => {
                let memory_buffer =
                    inkwell::memory_buffer::MemoryBuffer::create_from_memory_range_copy(
                        llvm_ir.source.as_bytes(),
                        self.path.as_str(),
                    );
                llvm.create_module_from_ir(memory_buffer)
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?
            }
            IR::ZKASM(ref zkasm) => {
                let build = compiler_llvm_context::build_assembly_text(
                    self.path.as_str(),
                    zkasm.source.as_str(),
                    metadata_hash,
                    debug_config.as_ref(),
                )?;
                return Ok(ContractBuild::new(
                    self.path,
                    identifier,
                    build,
                    metadata_json,
                    HashSet::new(),
                ));
            }
            _ => llvm.create_module(self.path.as_str()),
        };
        let mut context = compiler_llvm_context::Context::new(
            &llvm,
            module,
            optimizer,
            Some(project),
            include_metadata_hash,
            debug_config,
        );
        context.set_solidity_data(compiler_llvm_context::ContextSolidityData::default());
        match self.ir {
            IR::Yul(_) => {
                let yul_data = compiler_llvm_context::ContextYulData::new(is_system_mode);
                context.set_yul_data(yul_data);
            }
            IR::EVMLA(_) => {
                let evmla_data = compiler_llvm_context::ContextEVMLAData::new(version);
                context.set_evmla_data(evmla_data);
            }
            IR::LLVMIR(_) => {}
            IR::ZKASM(_) => {}
        }

        let factory_dependencies = self.drain_factory_dependencies();

        self.ir.declare(&mut context).map_err(|error| {
            anyhow::anyhow!(
                "The contract `{}` LLVM IR generator declaration pass error: {}",
                self.path,
                error
            )
        })?;
        self.ir.into_llvm(&mut context).map_err(|error| {
            anyhow::anyhow!(
                "The contract `{}` LLVM IR generator definition pass error: {}",
                self.path,
                error
            )
        })?;

        let build = context.build(self.path.as_str(), metadata_hash)?;

        Ok(ContractBuild::new(
            self.path,
            identifier,
            build,
            metadata_json,
            factory_dependencies,
        ))
    }
}

impl<D> WriteLLVM<D> for Contract
where
    D: compiler_llvm_context::Dependency + Clone,
{
    fn declare(&mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        self.ir.declare(context)
    }

    fn into_llvm(self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        self.ir.into_llvm(context)
    }
}
