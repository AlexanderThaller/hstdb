@_default:
    @just --list

# Generate README.md from template and clap output
generate-readme:
    cargo run --features generate-readme -- generate-readme
