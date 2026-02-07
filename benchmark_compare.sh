#!/bin/bash
# Sequence to create baseline "main" and compare current changes against it
# Note: If a baseline named "main" already exists, it will be overwritten.

# Step 1: Delete existing "main" baseline if it exists
BASELINE_DIR="target/criterion"
if [ -d "$BASELINE_DIR" ]; then
    echo "Step 1: Removing existing benchmark data at '$BASELINE_DIR'..."
    rm -rf "$BASELINE_DIR"
else
    echo "Step 1: No existing benchmark data found. Skipping cleanup."
fi

# Step 2: Stash current changes to save them
echo "Step 2: Stashing current changes..."
git stash push -m "Temporary stash for benchmark comparison"

# Step 3: Run benchmark and save as "main" baseline
echo "Step 3: Running benchmark to create 'main' baseline..."
cargo bench --package sim_core --bench performance -- --save-baseline main

# Step 4: Reapply your changes
echo "Step 4: Reapplying your changes..."
git stash pop

# Step 5: Run benchmark comparing against "main" baseline
echo "Step 5: Running benchmark comparing against 'main' baseline..."
cargo bench --package sim_core --bench performance -- --baseline main

echo "Done! Check the output above to see performance comparison."
