param([ValidateSet('normal', 'usb-probe')][string]$Profile = 'normal')
$ErrorActionPreference = 'Stop'
$terraRoot = Split-Path $PSScriptRoot -Parent
. (Join-Path $PSScriptRoot 'activate-local.ps1')
$firmware = Join-Path $terraRoot 'firmware'
$defaults = Join-Path $firmware 'sdkconfig.defaults'
$buildDirectory = Join-Path $firmware 'build'
$config = Join-Path $firmware 'sdkconfig'
if ($Profile -eq 'usb-probe') {
    $defaults += ';' + (Join-Path $firmware 'sdkconfig.usb-probe.defaults')
    $buildDirectory = Join-Path $firmware 'build-usb-probe'
    $config = Join-Path $firmware 'sdkconfig.usb-probe'
}
& idf.py -C $firmware -B $buildDirectory -D "SDKCONFIG=$config" -D "SDKCONFIG_DEFAULTS=$defaults" build size
if ($LASTEXITCODE -ne 0) { throw "Firmware build failed: $Profile" }
