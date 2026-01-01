# r1cs-optimizer

A static analysis and optimization tool for R1CS (Rank-1 Constraint Systems) compatible with arkworks.

## Overview

R1CS constraints are the foundation of zkSNARK proof systems like Groth16. This optimizer analyzes
and reduces constraint systems to minimize proof generation time and verifier costs.

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Your Circuit   │ ──► │  R1CS Matrix    │ ──► │  Optimized R1CS │
│  (arkworks)     │     │  (A, B, C)      │     │  (fewer constr) │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                              │
                              ▼
                        ┌─────────────────┐
                        │  Reduction      │
                        │  Reports        │
                        └─────────────────┘
```

## Architecture

The optimizer is built around the `ReductionPass` trait - the fundamental unit of optimization:

```rust
pub trait ReductionPass<F: PrimeField>: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;

    /// Scan the matrix for patterns this pass can optimize
    fn scan(&self, matrix: &ConstraintMatrix<F>) -> Vec<PatternMatch>;

    /// Apply the reduction, producing a smaller valid matrix
    fn reduce(&self, matrix: ConstraintMatrix<F>, matches: &[PatternMatch]) -> ConstraintMatrix<F>;

    /// Generate a report of what was found/reduced
    fn report(&self, matches: &[PatternMatch]) -> ReductionReport;
}
```

Each reduction pass:
1. **Scans** the constraint matrix for patterns with arbitrary complexity
2. **Transforms** these patterns into their optimal form
3. **Maintains** R1CS validity throughout the transformation
4. **Reports** what it found and reduced

## Reduction Passes

### 1. Deduplication Pass

Removes duplicate constraints that are structurally identical.

**Pattern:**
```
Constraint 17: (a + b) * 1 = c
Constraint 42: (a + b) * 1 = c  ← DUPLICATE
```

**Reduction:** Keep one copy, remove all duplicates.

**How it works:**
- Hash each constraint's (A_row, B_row, C_row) coefficients
- Group by hash, verify structural equality
- Remove duplicates, keeping the canonical (first) constraint

### 2. Constant Folding Pass

Removes constraints that only involve constants and can be verified at compile time.

**Pattern:**
```
Constraint: 5 * 3 = 15  ← Only uses Variable::One
```

**Reduction:** Verify the equation holds, then remove the constraint entirely.

**How it works:**
- Detect constraints where A, B, and C only reference Variable::One
- Evaluate the constant equation
- Remove if it holds (soundness preserved)

### 3. Linear Substitution Pass

Inlines simple variable definitions to eliminate intermediate constraints.

**Pattern:**
```
Constraint 1: 1 * (a + b) = x      ← x is defined as (a + b)
Constraint 2: x * y = z
```

**Reduction:**
```
Constraint 2: (a + b) * y = z      ← x substituted
[Constraint 1 removed]
```

**How it works:**
- Find constraints of form `1 * expr = var`
- Track where `var` is used
- Substitute `expr` for `var` in all uses
- Remove the definition constraint

### 4. Dead Variable Elimination Pass

Removes constraints that define variables never used in public outputs.

**Pattern:**
```
Constraint: a * b = unused    ← 'unused' never appears elsewhere
```

**Reduction:** Remove the entire constraint.

**How it works:**
- Build a usage graph: which variables are used where
- Identify variables defined but never used in A or B of any constraint
- Remove constraints that only define dead variables
- Note: Preserves all public inputs/outputs

### 5. Common Subexpression Detection Pass

Identifies repeated linear combinations across constraints (informational).

**Pattern:**
```
Constraint 10: (a + b) * (c + d) = e
Constraint 25: (a + b) * (x + y) = f   ← (a + b) recomputed
Constraint 40: (a + b) * (z + w) = g   ← (a + b) recomputed
```

**Report:** Suggests factoring out `(a + b)` to a single variable.

**Note:** This pass is informational only - it reports optimization opportunities but doesn't automatically apply them (would require circuit restructuring).

## Usage

### Basic Optimization

```rust
use ark_bn254::Fr;
use ark_relations::r1cs::ConstraintSystem;
use r1cs_optimizer::{Optimizer, OptimizerConfig};

// Create your circuit
let cs = ConstraintSystem::<Fr>::new_ref();
my_circuit.generate_constraints(cs.clone())?;
cs.finalize();

// Optimize
let result = Optimizer::from_cs(cs).optimize();

println!("Reduced {} → {} constraints ({:.2}% reduction)",
    result.original_constraints,
    result.final_constraints,
    result.reduction_percentage());

// Access the optimized matrix
let optimized_matrix = result.matrix;
```

### Configuration Options

```rust
// Safe config: only provably-safe reductions
let result = Optimizer::from_cs(cs)
    .with_config(OptimizerConfig::safe())
    .optimize();

// Aggressive config: all passes enabled
let result = Optimizer::from_cs(cs)
    .with_config(OptimizerConfig::aggressive())
    .optimize();

// Custom config
let config = OptimizerConfig {
    deduplicate: true,
    fold_constants: true,
    substitute_linear: false,  // Disable substitution
    eliminate_dead: false,     // Disable dead variable elimination
    detect_cse: true,
    max_iterations: 5,
};
let result = Optimizer::from_cs(cs)
    .with_config(config)
    .optimize();
```

### Analysis Only

```rust
// Get statistics without modifying
let optimizer = Optimizer::from_cs(cs);
let stats = optimizer.stats();

println!("Constraints: {}", stats.num_constraints);
println!("Variables:   {}", stats.num_variables);
println!("Linear:      {} ({:.1}%)",
    stats.linear_constraints,
    100.0 * stats.linear_constraints as f64 / stats.num_constraints as f64);
println!("Boolean:     {}", stats.boolean_constraints);
println!("Constant:    {}", stats.constant_constraints);

// Get reduction reports without applying
let reports = optimizer.analyze();
for report in reports {
    println!("{}: {} patterns found, {} potential savings",
        report.pass_name,
        report.patterns_found,
        report.estimated_savings);
}
```

### Using Individual Passes

```rust
use r1cs_optimizer::{ConstraintMatrix, DeduplicationPass, ReductionPass};

let matrix = ConstraintMatrix::from_cs(cs);
let pass = DeduplicationPass::new();

// Scan for patterns
let matches = pass.scan(&matrix);
println!("Found {} duplicate groups", matches.len());

// Apply reduction
let reduced = pass.reduce(matrix, &matches);
println!("Reduced to {} constraints", reduced.num_constraints());

// Or use the combined optimize method
let (reduced, report) = pass.optimize(matrix);
```

## Example Output

```
╔══════════════════════════════════════════════════════════════╗
║           R1CS OPTIMIZER - INVENTORY CIRCUITS                ║
╚══════════════════════════════════════════════════════════════╝

───────────────────────────────────────────────────────────────
  Circuit: StateTransition
───────────────────────────────────────────────────────────────

  BEFORE:
    Constraints:      7520
    Variables:        6410
    Linear:           1932 (25.7%)
    Boolean:             0
    Constant:            3
    Density:        0.3979%

  AFTER:
    Constraints:      7517
    Reduced:             3 (0.04%)

  PASSES:
    Constant Folding - 3 patterns, 3 savings
    Common Subexpression - 2 patterns, 3 savings
```

## Safety Guarantees

All reduction passes preserve:

1. **Soundness**: If the original R1CS accepts a witness, the reduced R1CS accepts the same witness
2. **Completeness**: If a witness satisfies the reduced R1CS, it satisfies the original
3. **Public I/O**: Public inputs and outputs are never eliminated

The `safe()` config preset enables only transformations that are provably correct:
- Deduplication (removing identical constraints)
- Constant folding (removing compile-time-verifiable constraints)

More aggressive passes (linear substitution, dead variable elimination) are available
but may require updating witness generation code.

## Compatibility

- arkworks 0.4.x
- Works with any `ConstraintSynthesizer<F>` implementation
- Field-agnostic (works with any `PrimeField`)

## License

MIT OR Apache-2.0
