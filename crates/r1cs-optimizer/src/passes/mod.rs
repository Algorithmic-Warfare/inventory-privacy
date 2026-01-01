//! Reduction passes for R1CS optimization.
//!
//! Each pass implements the `ReductionPass` trait, providing:
//! - Pattern scanning (detection)
//! - Reduction (transformation)
//! - Reporting (informative layer)
//!
//! # Available Passes
//!
//! | Pass | Pattern | Reduction |
//! |------|---------|-----------|
//! | `DeduplicationPass` | Identical constraints | Remove duplicates |
//! | `ConstantFoldingPass` | Compile-time constants | Verify and remove |
//! | `LinearSubstitutionPass` | `1 * expr = var` | Inline expression |
//! | `DeadVariablePass` | Unused variables | Remove definitions |
//! | `CommonSubexpressionPass` | Repeated terms | Factor out |

mod deduplication;
mod constant_folding;
mod linear_substitution;
mod dead_variable;
mod common_subexpression;

pub use deduplication::DeduplicationPass;
pub use constant_folding::ConstantFoldingPass;
pub use linear_substitution::LinearSubstitutionPass;
pub use dead_variable::DeadVariablePass;
pub use common_subexpression::CommonSubexpressionPass;

// Re-export the trait for convenience
pub use crate::reduction::ReductionPass;
