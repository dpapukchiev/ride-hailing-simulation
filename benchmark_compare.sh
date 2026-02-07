#!/bin/bash
# Sequence to create baseline "main" and compare current changes against it
# Note: If a baseline named "main" already exists, it will be overwritten.

# Step 1: Stash current changes to save them
echo "Step 1: Stashing current changes..."
git stash push -m "Temporary stash for benchmark comparison"

# Step 2: Run benchmark and save as "main" baseline
# Note: --save-baseline will overwrite existing "main" baseline if it exists
echo "Step 2: Running benchmark to create/overwrite 'main' baseline..."
echo "  (Warning: This will overwrite any existing 'main' baseline)"
cargo bench --package sim_core --bench performance -- --save-baseline main

# Step 3: Reapply your changes
echo "Step 3: Reapplying your changes..."
git stash pop

# Step 4: Run benchmark comparing against "main" baseline
echo "Step 4: Running benchmark comparing against 'main' baseline..."
cargo bench --package sim_core --bench performance -- --baseline main

echo "Done! Check the output above to see performance comparison."
