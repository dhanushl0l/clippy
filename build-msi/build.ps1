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

# Gen encryption key
$bytes = New-Object byte[] 24
[System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
$KEY = [Convert]::ToBase64String($bytes)
$env:KEY = $KEY 

# --- Build 32-bit ---
cargo build --release --target i686-pc-windows-msvc --bin clippy --bin clippy-gui
if ($?) {
    dotnet build build-msi -c Release -p:Platform=x86
    if ($?) {
        Copy-Item "build-msi/bin/x86/Release/en-US/clippy.msi" "build/clippy-$version-windows-x86.msi"
    }
}

# --- Build 64-bit ---
cargo build --release --target x86_64-pc-windows-msvc --bin clippy --bin clippy-gui
if ($?) {
    dotnet build build-msi -c Release -p:Platform=x64
    if ($?) {
        Copy-Item "build-msi/bin/x64/Release/en-US/clippy.msi" "build/clippy-$version-windows-x64.msi"
    }
}

# --- Build ARM64 ---
cargo build --release --target aarch64-pc-windows-msvc --bin clippy --bin clippy-gui
if ($?) {
    dotnet build build-msi -c Release -p:Platform=arm64
    if ($?) {
        Copy-Item "build-msi/bin/arm64/Release/en-US/clippy.msi" "build/clippy-$version-windows-arm64-$version.msi"
    }
}
