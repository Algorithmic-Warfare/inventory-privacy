# Inventory Privacy PoC - Development Commands
# Install just: cargo install just

# Use PowerShell on Windows
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# Default recipe - show help
default:
    @just --list

# Run all services with mprocs (recommended)
dev:
    mprocs

# Run individual services
sui:
    npm run start:sui

proof-server:
    npm run server:start

web:
    npm run client:dev

# Bootstrap: fund address and deploy contracts (run after sui is ready)
bootstrap:
    npm run bootstrap

# Just fund the address
fund:
    npm run fund

# Just deploy contracts (assumes already funded)
deploy:
    npm run deploy

# Build commands
build:
    cargo build --release

build-move:
    cd packages/inventory; sui move build

# Test commands
test:
    cargo test

test-move:
    cd packages/inventory; sui move test

# Clean build artifacts
clean:
    cargo clean
    if (Test-Path packages/inventory/build) { Remove-Item -Recurse -Force packages/inventory/build }
    if (Test-Path web/dist) { Remove-Item -Recurse -Force web/dist }

# Setup development environment
setup:
    cargo build
    cd web; npm install
    Write-Host "Setup complete!"
    Write-Host ""
    Write-Host "Workflow:"
    Write-Host "  1. Start services: just dev (or mprocs)"
    Write-Host "  2. Wait for sui-local to show 'SuiNode started'"
    Write-Host "  3. Run fund process in mprocs (press Enter on 'fund')"
    Write-Host "  4. Run deploy process in mprocs (press Enter on 'deploy')"
    Write-Host "  5. Open http://localhost:5173"

# Check if all tools are installed
doctor:
    Write-Host "Checking tools..."
    if (Get-Command sui -ErrorAction SilentlyContinue) { Write-Host "sui: OK" } else { Write-Host "sui: NOT FOUND" }
    if (Get-Command cargo -ErrorAction SilentlyContinue) { Write-Host "cargo: OK" } else { Write-Host "cargo: NOT FOUND" }
    if (Get-Command node -ErrorAction SilentlyContinue) { Write-Host "node: OK" } else { Write-Host "node: NOT FOUND" }
    if (Get-Command mprocs -ErrorAction SilentlyContinue) { Write-Host "mprocs: OK" } else { Write-Host "mprocs: NOT FOUND" }
    if (Get-Command bash -ErrorAction SilentlyContinue) { Write-Host "bash: OK" } else { Write-Host "bash: NOT FOUND (needed for scripts)" }
    Write-Host "Done."
