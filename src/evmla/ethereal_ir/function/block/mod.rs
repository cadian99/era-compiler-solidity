//!
//! The Ethereal IR block.
//!

pub mod element;

use std::collections::HashSet;

use num::Zero;

use crate::evmla::assembly::instruction::name::Name as InstructionName;
use crate::evmla::assembly::instruction::Instruction;

use self::element::stack::Stack as ElementStack;
use self::element::Element;

///
/// The Ethereal IR block.
///
#[derive(Debug, Clone)]
pub struct Block {
    /// The Solidity compiler version.
    pub solc_version: semver::Version,
    /// The block key.
    pub key: compiler_llvm_context::FunctionBlockKey,
    /// The block instance.
    pub instance: Option<usize>,
    /// The block elements relevant to the stack consistency.
    pub elements: Vec<Element>,
    /// The block predecessors.
    pub predecessors: HashSet<(compiler_llvm_context::FunctionBlockKey, usize)>,
    /// The initial stack state.
    pub initial_stack: ElementStack,
    /// The stack.
    pub stack: ElementStack,
    /// The extra block hashes for alternative routes.
    pub extra_hashes: Vec<md5::Digest>,
}

impl Block {
    /// The elements vector initial capacity.
    pub const ELEMENTS_VECTOR_DEFAULT_CAPACITY: usize = 64;
    /// The predecessors hashset initial capacity.
    pub const PREDECESSORS_HASHSET_DEFAULT_CAPACITY: usize = 4;

    ///
    /// Assembles a block from the sequence of instructions.
    ///
    pub fn try_from_instructions(
        solc_version: semver::Version,
        code_type: compiler_llvm_context::CodeType,
        slice: &[Instruction],
    ) -> anyhow::Result<(Self, usize)> {
        let mut cursor = 0;

        let tag: num::BigUint = match slice[cursor].name {
            InstructionName::Tag => {
                let tag = slice[cursor]
                    .value
                    .as_deref()
                    .expect("Always exists")
                    .parse()
                    .expect("Always valid");
                cursor += 1;
                tag
            }
            _ => num::BigUint::zero(),
        };

        let mut block = Self {
            solc_version: solc_version.clone(),
            key: compiler_llvm_context::FunctionBlockKey::new(code_type, tag),
            instance: None,
            elements: Vec::with_capacity(Self::ELEMENTS_VECTOR_DEFAULT_CAPACITY),
            predecessors: HashSet::with_capacity(Self::PREDECESSORS_HASHSET_DEFAULT_CAPACITY),
            initial_stack: ElementStack::new(),
            stack: ElementStack::new(),
            extra_hashes: vec![],
        };

        while cursor < slice.len() {
            let element: Element = Element::new(solc_version.clone(), slice[cursor].to_owned());
            block.elements.push(element);

            match slice[cursor].name {
                InstructionName::RETURN
                | InstructionName::REVERT
                | InstructionName::STOP
                | InstructionName::INVALID
                | InstructionName::JUMP => {
                    cursor += 1;
                    break;
                }
                InstructionName::Tag => {
                    break;
                }
                _ => {
                    cursor += 1;
                }
            }
        }

        Ok((block, cursor))
    }

    ///
    /// Inserts a predecessor tag.
    ///
    pub fn insert_predecessor(
        &mut self,
        key: compiler_llvm_context::FunctionBlockKey,
        instance: usize,
    ) {
        self.predecessors.insert((key, instance));
    }
}

impl<D> compiler_llvm_context::WriteLLVM<D> for Block
where
    D: compiler_llvm_context::Dependency + Clone,
{
    fn into_llvm(self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        context.set_code_type(self.key.code_type);

        for element in self.elements.into_iter() {
            element.into_llvm(context)?;
        }

        Ok(())
    }
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "block_{}/{}: {}",
            self.key,
            self.instance.unwrap_or_default(),
            if self.predecessors.is_empty() {
                "".to_owned()
            } else {
                format!(
                    "(predecessors: {})",
                    self.predecessors
                        .iter()
                        .map(|(key, instance)| format!("{}/{}", key, instance))
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            },
        )?;
        for element in self.elements.iter() {
            writeln!(f, "    {element}")?;
        }
        Ok(())
    }
}
