# Local build — Windows portable.
#   dist/dictz/ (--onedir) + dist/dictz-<version>-windows-<arch>.zip
$ErrorActionPreference = "Stop"
Set-Location "$PSScriptRoot/.."

$version = (Select-String -Path "dictz/__init__.py" -Pattern '__version__ = "([^"]+)"').Matches[0].Groups[1].Value
$arch = $env:PROCESSOR_ARCHITECTURE.ToLower()
if ($arch -eq "amd64") { $arch = "x86_64" }

Write-Host "==> Building dictz $version on Windows/$arch"

python -m pip install --upgrade --quiet pyinstaller
python -m pip install --upgrade --quiet -e .

Remove-Item -Recurse -Force build, dist -ErrorAction SilentlyContinue
pyinstaller packaging/pyinstaller/dictz.spec --noconfirm --clean

$zipName = "dictz-$version-windows-$arch.zip"
Push-Location dist
Compress-Archive -Path dictz -DestinationPath $zipName -Force
Pop-Location

Write-Host "==> Windows portable: dist/$zipName"
Get-ChildItem dist/*.zip | Format-Table Name, Length
