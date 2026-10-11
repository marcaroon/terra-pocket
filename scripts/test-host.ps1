$ErrorActionPreference = 'Stop'
$terraRoot = Split-Path $PSScriptRoot -Parent
Push-Location $terraRoot
try {
    New-Item -ItemType Directory -Path .tools -Force | Out-Null
    $compiler = if (Test-Path .tools/w64devkit/bin/gcc.exe) { '.tools/w64devkit/bin/gcc.exe' } else { 'gcc' }
    $checks = @(
        @('time', 'firmware/tests/test_time_utils.c', 'firmware/main/util/time_utils.c'),
        @('input', 'firmware/tests/test_input_logic.c', 'firmware/main/input/input_logic.c'),
        @('disk', 'firmware/tests/test_disk_inspect.c', 'firmware/main/storage/disk_inspect.c'),
        @('protocol', 'firmware/tests/test_protocol.c', 'firmware/main/protocol/protocol.c')
    )
    foreach ($check in $checks) {
        $binary = Join-Path $terraRoot ".tools/test-$($check[0]).exe"
        & $compiler -std=c11 -Wall -Wextra -Werror -I firmware/main $check[1] $check[2] -o $binary
        if ($LASTEXITCODE -ne 0) { throw "Compile failed: $($check[0])" }
        & $binary docs/protocol-test-vectors
        if ($LASTEXITCODE -ne 0) { throw "Test failed: $($check[0])" }
    }
} finally { Pop-Location }
