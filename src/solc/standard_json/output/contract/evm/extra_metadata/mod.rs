//!
//! The `solc --standard-json` output contract EVM extra metadata.
//!

pub mod recursive_function;

use serde::Deserialize;
use serde::Serialize;

use self::recursive_function::RecursiveFunction;

///
/// The `solc --standard-json` output contract EVM extra metadata.
///
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtraMetadata {
    /// The list of recursive functions.
    #[serde(default = "Vec::new")]
    pub recursive_functions: Vec<RecursiveFunction>,
}

impl ExtraMetadata {
    ///
    /// Returns the recursive function reference for the specified tag.
    ///
    pub fn get(
        &self,
        block_key: &compiler_llvm_context::FunctionBlockKey,
    ) -> Option<&RecursiveFunction> {
        for function in self.recursive_functions.iter() {
            match block_key.code_type {
                compiler_llvm_context::CodeType::Deploy => {
                    if let Some(creation_tag) = function.creation_tag {
                        if num::BigUint::from(creation_tag) == block_key.tag {
                            return Some(function);
                        }
                    }
                }
                compiler_llvm_context::CodeType::Runtime => {
                    if let Some(runtime_tag) = function.runtime_tag {
                        if num::BigUint::from(runtime_tag) == block_key.tag {
                            return Some(function);
                        }
                    }
                }
            }
        }

        None
    }
}
