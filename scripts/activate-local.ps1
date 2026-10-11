# Dot-source from any directory. A convenience for this checkout's optional tools.
# Does not change persistent user/system PATH.
$terraRoot = Split-Path $PSScriptRoot -Parent
$terraTools = Join-Path $terraRoot '.tools'
if (-not $env:IDF_PY_BUILD_JOBS) {
    $env:IDF_PY_BUILD_JOBS = '4'
}
$terraRust = Join-Path $terraTools 'rustup'
$terraCargo = Join-Path $terraTools 'cargo'
if (Test-Path (Join-Path $terraCargo 'bin/cargo.exe')) {
    $env:RUSTUP_HOME = $terraRust
    $env:CARGO_HOME = $terraCargo
    $env:PATH = (Join-Path $terraCargo 'bin') + ';' + $env:PATH
}
$terraMingw = Join-Path $terraTools 'llvm-mingw-20260908-ucrt-x86_64/bin'
if (Test-Path $terraMingw) {
    $env:PATH = $terraMingw + ';' + $env:PATH
}
$terraGcc = Join-Path $terraTools 'w64devkit/bin'
if (Test-Path $terraGcc) {
    $env:PATH = $terraGcc + ';' + $env:PATH
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $terraGcc 'gcc.exe'
    if (-not $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS) {
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = '-C link-self-contained=yes'
    }
}
$terraActivation = Join-Path $terraTools 'espressif/tools/activate_idf_v6.0.3.ps1'
if (Test-Path $terraActivation) {
    . $terraActivation
} else {
    $terraIdf = Join-Path $terraTools 'espressif/v6.0.3/esp-idf'
    $terraPython = Join-Path $terraTools 'espressif/python_env/idf6.0_py3.12_env/Scripts'
    if (Test-Path (Join-Path $terraPython 'python.exe')) {
        $env:IDF_TOOLS_PATH = Join-Path $terraTools 'espressif'
        $env:PATH = $terraPython + ';' + $env:PATH
        . (Join-Path $terraIdf 'export.ps1')
    } else {
        Write-Warning 'Local ESP-IDF activation not found. Activate your ESP-IDF 6.0.x environment separately.'
    }
}
