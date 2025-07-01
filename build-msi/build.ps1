$env:KEY = ([Convert]::ToBase64String((1..24 | ForEach-Object { [byte](Get-Random -Max 256) }))).Substring(0,32)

# 32-bit
cargo build --release --target i686-pc-windows-msvc --bin clippy --bin clippy-gui
dotnet build build-msi -c Release -p:Platform=x86

# 64-bit
cargo build --release --target x86_64-pc-windows-msvc --bin clippy --bin clippy-gui
dotnet build build-msi -c Release -p:Platform=x64

# ARM64
# cargo build --release --target aarch64-pc-windows-msvc --bin clippy --bin clippy-gui
# dotnet build build-msi -c Release -p:Platform=arm64