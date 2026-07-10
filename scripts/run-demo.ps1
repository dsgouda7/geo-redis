Write-Host "GeoRedis — starting demos..." -ForegroundColor Cyan

# ── prerequisites check ───────────────────────────────────────────────────
foreach ($cmd in @("cargo", "node", "npm", "docker")) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Error "Required tool not found: $cmd"
        exit 1
    }
}

Set-Location $PSScriptRoot\..

# ── .env ──────────────────────────────────────────────────────────────────
if (-not (Test-Path .env)) {
    Copy-Item config\.env.example .env
    Write-Host "Created .env from config/.env.example" -ForegroundColor Green
}

# ── Redis via Docker ──────────────────────────────────────────────────────
Write-Host "Starting Redis..." -ForegroundColor Yellow
docker compose -f demo/docker-compose.yml up -d
Start-Sleep -Seconds 2

# ── UI node_modules ───────────────────────────────────────────────────────
if (-not (Test-Path demo/ui/node_modules)) {
    Write-Host "Installing UI dependencies (first run)..." -ForegroundColor Yellow
    Push-Location demo/ui; npm install; Pop-Location
}

# ── Build Rust binaries ───────────────────────────────────────────────────
Write-Host "Building backends (first build may take ~60s)..." -ForegroundColor Yellow
cargo build --release -p georedis-demo -p georedis-adsb
if ($LASTEXITCODE -ne 0) { Write-Error "Build failed"; exit 1 }

# ── Load .env into current session ───────────────────────────────────────
Get-Content .env | Where-Object { $_ -match "^\s*[^#]\S+=\S" } | ForEach-Object {
    $k, $v = $_ -split "=", 2
    [System.Environment]::SetEnvironmentVariable($k.Trim(), $v.Trim(), "Process")
}

# ── Demo server — port 3000, Redis DB 0 (OpenSky) ────────────────────────
Write-Host "Starting OpenSky demo server  →  :3000" -ForegroundColor Yellow
$env:SERVER_PORT = "3000"; $env:SQLITE_PATH = "georedis.db"
$env:REDIS_URL   = if ($env:REDIS_URL) { $env:REDIS_URL } else { "redis://127.0.0.1:6379" }
$p0 = Start-Process -FilePath ".\target\release\georedis-demo.exe" `
    -RedirectStandardOutput ".\target\demo-stdout.log" `
    -RedirectStandardError  ".\target\demo-stderr.log" `
    -PassThru -NoNewWindow

# ── ADSB server — port 3001, Redis DB 1 ──────────────────────────────────
Write-Host "Starting ADSB demo server     →  :3001" -ForegroundColor Yellow
$env:SERVER_PORT = "3001"; $env:SQLITE_PATH = "georedis-adsb.db"
$env:REDIS_URL   = "redis://127.0.0.1:6379/1"
$p1 = Start-Process -FilePath ".\target\release\georedis-adsb.exe" `
    -RedirectStandardOutput ".\target\adsb-stdout.log" `
    -RedirectStandardError  ".\target\adsb-stderr.log" `
    -PassThru -NoNewWindow

Start-Sleep -Seconds 3

# ── Vite UI dev servers ───────────────────────────────────────────────────
Write-Host "Starting UI dev servers..." -ForegroundColor Yellow
Push-Location demo/ui
$ui0 = Start-Process -FilePath "npm" -ArgumentList "run","dev"      -PassThru -NoNewWindow
$ui1 = Start-Process -FilePath "npm" -ArgumentList "run","dev:adsb" -PassThru -NoNewWindow
Pop-Location

Start-Sleep -Seconds 3

Write-Host ""
Write-Host "  ┌────────────────────────────────────────────────┐" -ForegroundColor Cyan
Write-Host "  │  OpenSky tracker  →  http://localhost:5173     │" -ForegroundColor Cyan
Write-Host "  │  ADSB demo        →  http://localhost:5174     │" -ForegroundColor Cyan
Write-Host "  └────────────────────────────────────────────────┘" -ForegroundColor Cyan
Write-Host ""
Write-Host "Logs: target/demo-stdout.log  target/adsb-stdout.log" -ForegroundColor DarkGray
Write-Host "Press Ctrl+C to stop everything." -ForegroundColor Gray

try {
    Wait-Process -Id $p0.Id
} finally {
    foreach ($p in @($p0, $p1, $ui0, $ui1)) {
        Stop-Process -Id $p.Id -ErrorAction SilentlyContinue
    }
    docker compose -f demo/docker-compose.yml down
    Write-Host "Stopped." -ForegroundColor Gray
}

