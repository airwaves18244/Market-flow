```markdown
# Market-flow Development Patterns

> Auto-generated skill from repository analysis

## Overview
This skill teaches the core development patterns and conventions used in the Market-flow Rust codebase. It covers file organization, commit message standards, import/export styles, and testing patterns, providing a reference for consistent and effective collaboration.

## Coding Conventions

### File Naming
- **Convention:** PascalCase for file names.
- **Example:**  
  `OrderBook.rs`, `MarketFlowEngine.rs`

### Import Style
- **Convention:** Use relative imports within the crate.
- **Example:**
  ```rust
  use crate::OrderBook;
  use super::MarketFlowEngine;
  ```

### Export Style
- **Convention:** Use named exports for modules and items.
- **Example:**
  ```rust
  pub struct OrderBook { /* ... */ }
  pub fn process_order() { /* ... */ }
  ```

### Commit Messages
- **Convention:** Conventional commits with `feat` prefix.
- **Example:**  
  `feat: add order matching logic to OrderBook`

## Workflows

### Feature Development
**Trigger:** When adding a new feature or module  
**Command:** `/feature-dev`

1. Create a new file using PascalCase (e.g., `NewFeature.rs`)
2. Implement the feature using relative imports for dependencies
3. Export structs, enums, or functions using named exports
4. Write or update corresponding test files (`*.test.rs`)
5. Commit changes with a message starting with `feat:`, followed by a concise description

### Code Importing
**Trigger:** When referencing code from another module  
**Command:** `/import-module`

1. Use relative imports (`use crate::ModuleName`) to bring in dependencies
2. Ensure the imported module uses named exports

### Testing
**Trigger:** When writing or running tests  
**Command:** `/run-tests`

1. Create or update test files matching the pattern `*.test.rs`
2. Write tests for public functions and modules
3. Run tests using the Rust test runner (`cargo test`)

## Testing Patterns

- **Test File Naming:** Use `*.test.rs` for test files.
- **Test Structure:** Place tests in the same directory as the module or in a dedicated `tests` folder.
- **Example:**
  ```rust
  // OrderBook.test.rs
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_order_matching() {
          // Test logic here
      }
  }
  ```
- **Test Runner:** Use `cargo test` to execute tests.

## Commands
| Command        | Purpose                                      |
|----------------|----------------------------------------------|
| /feature-dev   | Start a new feature/module development       |
| /import-module | Import a module using relative imports       |
| /run-tests     | Run all tests in the codebase                |
```
