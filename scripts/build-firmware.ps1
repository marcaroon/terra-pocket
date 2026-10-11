param([ValidateSet('normal', 'usb-probe')][string]$Profile = 'normal')
$ErrorActionPreference = 'Stop'
$terraRoot = Split-Path $PSScriptRoot -Parent
$firmware = Join-Path $terraRoot 'firmware'
$defaults = Join-Path $firmware 'sdkconfig.defaults'
$buildDirectory = Join-Path $firmware 'build'
$config = Join-Path $firmware 'sdkconfig'
if ($Profile -eq 'usb-probe') {
    $defaults += ';' + (Join-Path $firmware 'sdkconfig.usb-probe.defaults')
    $buildDirectory = Join-Path $firmware 'build-usb-probe'
    $config = Join-Path $firmware 'sdkconfig.usb-probe'
}
# ESP-IDF tools print progress on stderr. Windows PowerShell turns redirected
# native stderr into error records, so 'Stop' would abort a successful build
# whenever output is captured (for example `| Tee-Object`). Native steps are
# checked through $LASTEXITCODE instead.
$ErrorActionPreference = 'Continue'
. (Join-Path $PSScriptRoot 'activate-local.ps1')
& idf.py -C $firmware -B $buildDirectory -D "SDKCONFIG=$config" -D "SDKCONFIG_DEFAULTS=$defaults" build size
if ($LASTEXITCODE -ne 0) { throw "Firmware build failed: $Profile" }
