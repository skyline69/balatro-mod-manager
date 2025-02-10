#Requires -Version 7

# Enable ANSI color support
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

# Colors
$RED = "`e[91m"
$GREEN = "`e[92m"
$YELLOW = "`e[93m"
$BLUE = "`e[94m"
$CYAN = "`e[38;2;61;181;255m"
$NC = "`e[0m"

Write-Host $CYAN
@"
    ____  __  _____  ___            ____           __        ____
   / __ )/  |/  /  |/  /           /  _/___  _____/ /_____ _/ / /
  / __  / /|_/ / /|_/ /  ______    / // __ \/ ___/ __/ __ `/ / /
 / /_/ / /  / / /  / /  /_____/  _/ // / / (__  ) /_/ /_/ / / /
/_____/_/  /_/_/  /_/           /___/_/ /_/____/\__/\__,_/_/_/
"@
Write-Host $NC

Write-Host "${GREEN}Balatro Mod Manager Builder${NC}"
Write-Host "----------------------------------------"
Write-Host "Build started at $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"

# OS check
if ($env:OS -ne "Windows_NT") {
    Write-Host "${RED}Error: This builder is for Windows only.${NC}"
    exit 1
}

# Dependency checks
$deps = @(
    @{Name="git"; Url="https://git-scm.com/downloads"},
    @{Name="cargo"; Url="https://www.rust-lang.org/tools/install"},
    @{Name="deno"; Url="https://deno.land/#installation"},
    @{Name="cargo-tauri"; Url="https://crates.io/crates/tauri-cli"}
)

Write-Host "${YELLOW}Checking dependencies...${NC}"
foreach ($dep in $deps) {
    try {
        $null = Get-Command $dep.Name -ErrorAction Stop
    } catch {
        Write-Host "${RED}Error: $($dep.Name) not found. Please install first.${NC}"
        Write-Host "${BLUE}$($dep.Url)${NC}"
        exit 1
    }
}

# Create temp directory
$BUILD_DIR = Join-Path $env:TEMP "balatro-mod-manager-$(Get-Date -Format 'yyyyMMddHHmmss')"
Write-Host "${YELLOW}Creating temporary build directory: ${BUILD_DIR}${NC}"
New-Item -Path $BUILD_DIR -ItemType Directory | Out-Null

# Clone repository
Write-Host "${YELLOW}1. Cloning repository...${NC}"
git clone https://github.com/skyline69/balatro-mod-manager.git (Join-Path $BUILD_DIR "balatro-mod-manager")
if (-not $?) {
    Write-Host "${RED}Git clone failed${NC}"
    Remove-Item $BUILD_DIR -Recurse -Force
    exit 1
}

# Build process
try {
    Set-Location (Join-Path $BUILD_DIR "balatro-mod-manager")

    Write-Host "${YELLOW}2. Installing deno dependencies...${NC}"
    deno install --allow-scripts
    if (-not $?) { throw "Deno install failed" }

    Write-Host "${YELLOW}3. Building frontend...${NC}"
    deno task build
    if (-not $?) { throw "Frontend build failed" }

    Write-Host "${YELLOW}4. Building Rust backend...${NC}"
    Set-Location src-tauri
    $env:SKIP_BUILD_SCRIPT = "1"
    cargo build --release
    if (-not $?) { throw "Cargo build failed" }

    Set-Location ..
    Write-Host "${YELLOW}5. Creating app bundle...${NC}"
    cargo tauri build
    if (-not $?) { throw "Tauri build failed" }
}
catch {
    Write-Host "${RED}$_${NC}"
    Remove-Item $BUILD_DIR -Recurse -Force
    exit 1
}

# Cleanup
Write-Host "${YELLOW}6. Cleaning up...${NC}"
Remove-Item $BUILD_DIR -Recurse -Force

Write-Host "${GREEN}Installation completed successfully!${NC}"
Write-Host ""
Write-Host "${YELLOW}Note: Windows SmartScreen might block first execution -"
Write-Host "right-click the .exe and select 'Run anyway'${NC}"

