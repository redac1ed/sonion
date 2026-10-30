$ErrorActionPreference = "Stop"

$repo = "redac1ed/sonion"
$file = "sonion-x86_64-pc-windows-msvc.zip"
$url = "https://github.com/$repo/releases/latest/download/$file"
$dir = Join-Path $env:LOCALAPPDATA "sonion\bin"
$tmp = Join-Path $env:TEMP $file

New-Item -ItemType Directory -Force $dir | Out-Null
Write-Host "downloading $url"
Invoke-WebRequest $url -OutFile $tmp
Expand-Archive $tmp -DestinationPath $dir -Force
Remove-Item $tmp

$path = [Environment]::GetEnvironmentVariable("Path", "User")
if ($path -notlike "*$dir*") {
    [Environment]::SetEnvironmentVariable("Path", "$path;$dir", "User")
    Write-Host "added $dir to PATH, restart your terminal"
}
Write-Host "installed sonion to $dir\sonion.exe"