#!/bin/bash
set -euo pipefail

deno run -A \
    npm:typescript-json-schema@0.65.1 \
    ./schema/cargo-config.schema.d.mts CargoConfigSchema \
    --noExtraProps -o ./schema/cargo-config.schema.json
