param([string]$Action = 'build')
$ErrorActionPreference = 'Stop'
if (Test-Path 'C:\Cursor\Project\tools\rust\cargo\bin\cargo.exe') {
    $env:RUSTUP_HOME = 'C:\Cursor\Project\tools\rust\rustup'
    $env:CARGO_HOME = 'C:\Cursor\Project\tools\rust\cargo'
    $env:Path = "$env:CARGO_HOME\bin;$env:Path"
}
Push-Location $PSScriptRoot
try {
    switch ($Action) {
        'check' { cargo check --locked }
        'test' { cargo test --locked }
        'fmt' { cargo fmt }
        'lint' { cargo clippy --locked --all-targets -- -D warnings }
        'build' { cargo build --locked --release }
        default { throw "Unknown action: $Action" }
    }
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed: $LASTEXITCODE" }
} finally { Pop-Location }
