alias p := preflight

_default:
    @just --list

# Lint markdown (check only)
fmt-md:
    bunx markdownlint-cli2 "**/*.md" "**/*.mdx"

# Lint markdown with auto-fix
fmt-md-fix:
    bunx markdownlint-cli2 --fix "**/*.md" "**/*.mdx"

# Run preflight checks
preflight:
    @echo "Preflight check..."
    @just fmt-md