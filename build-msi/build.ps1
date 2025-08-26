# Read version from Cargo.toml
$tomlPath = "clippy/Cargo.toml"
$versionLine = Get-Content $tomlPath | Where-Object { $_ -match '^version\s*=' }
$version = ($versionLine -split '"')[1]

# Update Version in Package.wxs
$wxsPath = "build-msi/Package.wxs"
(Get-Content $wxsPath) | ForEach-Object {
    if ($_ -match '<Package\b.*Version="[\d\.]*"') {
        $_ -replace 'Version="[\d\.]*"', "Version=`"$version.0`""
    } else {
        $_
    }
} | Set-Content $wxsPath

# Ensure output dir exists
New-Item -ItemType Directory -Path "build" -Force | Out-Null

# Detect architecture
$arch = if ([Environment]::Is64BitOperatingSystem) {
    if ($env:PROCESSOR_ARCHITECTURE -eq "AMD64") { "x64" }
    elseif ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" }
    else { "x86" }  # fallback
} else { "x86" }

Write-Host "Detected architecture: $arch"

# Build function
function Build-Clippy($target, $platform) {
    Write-Host "Building for $target ($platform)..."
    cargo build --release --target $target --bin clippy --bin clippy-gui
    if ($?) {
        dotnet build build-msi -c Release -p:Platform=$platform
        if ($?) {
            $msiName = "clippy-$platform-$version.msi"
            Copy-Item "build-msi/bin/$platform/Release/en-US/clippy.msi" "build/$msiName"
            Write-Host "Build successful: build/$msiName"
        }
    }
}

switch ($arch) {
    "x86"   { Build-Clippy "i686-pc-windows-msvc" "x86" }
    "x64"   { Build-Clippy "x86_64-pc-windows-msvc" "x64" }
    "arm64" { Build-Clippy "aarch64-pc-windows-msvc" "arm64" }
    default { Write-Host "Unknown architecture: $arch" }
}
