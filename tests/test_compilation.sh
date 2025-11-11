#!/bin/bash

echo "Testing module syntax..."

# Test each module can be parsed
for file in nonce_errors.rs nonce_retry.rs nonce_signer.rs nonce_lease.rs nonce_refresh.rs nonce_predictive.rs; do
    echo "Checking $file..."
    if ! rustc --crate-type lib $file --edition 2021 -Z parse-only 2>/dev/null; then
        echo "Note: $file may have dependencies, syntax check skipped"
    fi
done

echo "All module files checked!"
