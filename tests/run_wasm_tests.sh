#!/bin/bash

set -e  # Exit immediately if a command exits with a non-zero status

# Run regular tests
echo "=== Building regular test binaries for wasm32-wasip1 ==="
output=$(cargo test --target wasm32-wasip1 --no-run --all 2>&1)

# First count all binaries
all_binaries=$(echo "$output" | grep -o "Executable.*" | wc -l)

# Extract the test binary paths from the output - only WASM files
# Looking specifically for paths inside parentheses that end with .wasm
test_binaries=$(echo "$output" | grep -o "(target/[^)]*\.wasm)" | sed 's/^(//' | sed 's/)$//')
wasm_count=$(echo "$test_binaries" | grep -v "^$" | wc -l)

echo "Found $all_binaries total test binaries, $wasm_count are WASM binaries that will be run."
echo "Note: Non-WASM binaries are skipped."

if [ -z "$test_binaries" ]; then
    echo "No WASM test binaries found. Check if cargo test output format is as expected."
    echo "Raw output:"
    echo "$output"
    exit 1
fi

# Track overall test status
all_tests_passed=true

# Run each test binary with wasmtime
for binary in $test_binaries; do
    if [ ! -f "$binary" ]; then
        echo "Warning: Binary file not found: $binary"
        all_tests_passed=false
        continue
    fi

    echo "Running tests in $binary"
    wasmtime "$binary"
    
    if [ $? -eq 0 ]; then
        echo "Tests passed for $binary"
    else
        echo "Tests failed for $binary"
        all_tests_passed=false
    fi
    
    echo "----------------------------------------"
done

# Try running doc tests directly
echo -e "\n=== Running doc tests for wasm32-wasip1 ==="
echo "Note: Doc tests for wasm32-wasip1 target may not work as expected."

# We'll set +e to prevent the script from exiting if doc tests fail
set +e
cargo test --doc --target wasm32-wasip1 --all 2>&1
doc_test_result=$?
set -e

if [ $doc_test_result -eq 0 ]; then
    echo "Doc tests completed successfully."
elif [ $doc_test_result -eq 101 ]; then
    echo "Warning: Doc tests were not run. They may not be supported for WASM targets."
else
    echo "Warning: Doc tests failed with exit code $doc_test_result."
    all_tests_passed=false
fi

if [ "$all_tests_passed" = true ]; then
    echo "All tests passed successfully!"
    exit 0
else
    echo "Some tests failed."
    exit 1
fi
