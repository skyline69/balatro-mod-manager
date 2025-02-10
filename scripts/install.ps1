#Requires -Version 5

# Colors replaced with Write-Host parameters
$RED = "Red"
$GREEN = "Green"
$YELLOW = "Yellow"
$BLUE = "Blue"
$CYAN = "Cyan"

Write-Host @"
    ____  __  _____  ___            ____           __        ____
   / __ )/  |/  /  |/  /           /  _/___  _____/ /_____ _/ / /
  / __  / /|_/ / /|_/ /  ______    / // __ \/ ___/ __/ __ `/ / /
 / /_/ / /  / / /  / /  /_____/  _/ // / / (__  ) /_/ /_/ / / /
/_____/_/  /_/_/  /_/           /___/_/ /_/____/\__/\__,_/_/_/
"@ -ForegroundColor $CYAN

Write-Host "Balatro Mod Manager Builder" -ForegroundColor $GREEN
Write-Host "----------------------------------------"
Write-Host "Build started at $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"

# OS check
if ($env:OS -ne "Windows_NT") {
    Write-Host "Error: This builder is for Windows only." -ForegroundColor $RED
    exit 1
}

# Dependency checks
$deps = @(
    @{Name="git"; Url="https://git-scm.com/downloads"},
    @{Name="cargo"; Url="https://www.rust-lang.org/tools/install"},
    @{Name="deno"; Url="https://deno.land/#installation"},
    @{Name="cargo-tauri"; Url="https://crates.io/crates/tauri-cli"}
)

Write-Host "Checking dependencies..." -ForegroundColor $YELLOW
foreach ($dep in $deps) {
    if (-not (Get-Command $dep.Name -ErrorAction SilentlyContinue)) {
        Write-Host "Error: $($dep.Name) not found. Please install first." -ForegroundColor $RED
        Write-Host $dep.Url -ForegroundColor $BLUE
        exit 1
    }
}

# Create temp directory
$BUILD_DIR = Join-Path $env:TEMP "balatro-mod-manager-$(Get-Date -Format 'yyyyMMddHHmmss')"
Write-Host "Creating temporary build directory: ${BUILD_DIR}" -ForegroundColor $YELLOW
New-Item -Path $BUILD_DIR -ItemType Directory -Force | Out-Null

# Clone repository
Write-Host "1. Cloning repository..." -ForegroundColor $YELLOW
git clone https://github.com/skyline69/balatro-mod-manager.git (Join-Path $BUILD_DIR "balatro-mod-manager")
if ($LASTEXITCODE -ne 0) {
    Write-Host "Git clone failed" -ForegroundColor $RED
    Remove-Item $BUILD_DIR -Recurse -Force
    exit 1
}

# Build process
try {
    Set-Location (Join-Path $BUILD_DIR "balatro-mod-manager")

    Write-Host "2. Installing deno dependencies..." -ForegroundColor $YELLOW
    deno install --allow-scripts
    if ($LASTEXITCODE -ne 0) { throw "Deno install failed" }

    Write-Host "3. Building frontend..." -ForegroundColor $YELLOW
    deno task build
    if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }

    Write-Host "4. Building Rust backend..." -ForegroundColor $YELLOW
    Set-Location src-tauri
    $env:SKIP_BUILD_SCRIPT = "1"
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed" }

    Set-Location ..
    Write-Host "5. Creating app bundle..." -ForegroundColor $YELLOW
    cargo tauri build
    if ($LASTEXITCODE -ne 0) { throw "Tauri build failed" }
}
catch {
    Write-Host $_ -ForegroundColor $RED
    Remove-Item $BUILD_DIR -Recurse -Force
    exit 1
}

Write-Host "Installation completed successfully!" -ForegroundColor $GREEN
Write-Host ""
Write-Host "Note: Windows SmartScreen might block first execution -`nright-click the .exe and select 'Run anyway'" -ForegroundColor $YELLOW
