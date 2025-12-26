# PowerShell script for Windows users to run e2e tests
# Run this from the repository root

$ErrorActionPreference = "Stop"

Write-Host "=== Inventory Privacy E2E Test ===" -ForegroundColor Cyan
Write-Host ""

# Check for required tools
$suiPath = Get-Command sui -ErrorAction SilentlyContinue
if (-not $suiPath) {
    Write-Host "Error: sui CLI not found. Please install it first:" -ForegroundColor Red
    Write-Host ""
    Write-Host "  # Download from GitHub releases:"
    Write-Host "  https://github.com/MystenLabs/sui/releases"
    Write-Host ""
    Write-Host "  # Or build from source:"
    Write-Host "  cargo install --locked --git https://github.com/MystenLabs/sui.git --branch devnet sui"
    exit 1
}

Write-Host "Sui version: $(sui --version)"
Write-Host ""

# Step 1: Build Rust crates
Write-Host "Step 1: Building Rust crates..." -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Step 2: Run Rust tests
Write-Host ""
Write-Host "Step 2: Running Rust tests..." -ForegroundColor Yellow
cargo test --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Step 3: Export verifying keys
Write-Host ""
Write-Host "Step 3: Exporting verifying keys..." -ForegroundColor Yellow
cargo run --release --bin export-vks
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Step 4: Build Move package
Write-Host ""
Write-Host "Step 4: Building Move package..." -ForegroundColor Yellow
Push-Location packages/inventory
sui move build
$buildResult = $LASTEXITCODE
Pop-Location
if ($buildResult -ne 0) { exit $buildResult }

# Step 5: Run Move tests
Write-Host ""
Write-Host "Step 5: Running Move tests..." -ForegroundColor Yellow
Push-Location packages/inventory
sui move test
$testResult = $LASTEXITCODE
Pop-Location
if ($testResult -ne 0) { exit $testResult }

# Step 6: Build web frontend
Write-Host ""
Write-Host "Step 6: Building web frontend..." -ForegroundColor Yellow
Push-Location web
npm install
npm run build
$webResult = $LASTEXITCODE
Pop-Location
if ($webResult -ne 0) { exit $webResult }

Write-Host ""
Write-Host "=== All E2E Tests Passed ===" -ForegroundColor Green
Write-Host ""
Write-Host "To run the full system locally:"
Write-Host ""
Write-Host "  1. Start local Sui network (in a separate terminal):"
Write-Host "     sui start --with-faucet"
Write-Host ""
Write-Host "  2. Deploy contracts:"
Write-Host "     ./scripts/deploy-local.sh"
Write-Host ""
Write-Host "  3. Start proof server:"
Write-Host "     cargo run --release -p inventory-proof-server"
Write-Host ""
Write-Host "  4. Start web frontend:"
Write-Host "     cd web && npm run dev"
Write-Host ""
