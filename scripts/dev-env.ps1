# Dot-source before any cargo command: `. ./scripts/dev-env.ps1`
# Imports the VS x64 env + LLVM/Ninja toolchain vars (BUILD.md, "Windows release
# build"). Without this, whisper-rs-sys' CMake build breaks on this toolchain.
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $vsPath = & $vswhere -latest -property installationPath
    $vsDevCmd = Join-Path $vsPath "Common7\Tools\VsDevCmd.bat"
    if (Test-Path $vsDevCmd) {
        $tmp = [IO.Path]::GetTempFileName()
        cmd.exe /c "call `"$vsDevCmd`" -arch=x64 > nul && set > `"$tmp`""
        Get-Content $tmp | ForEach-Object {
            # [^=]+ : cmd emits hidden "=C:=..." vars whose empty name would throw
            if ($_ -match '^([^=]+)=(.*)$') {
                [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process')
            }
        }
        Remove-Item $tmp -Force
    }
}

$env:CC = 'clang-cl'; $env:CXX = 'clang-cl'
$env:CMAKE_GENERATOR = 'Ninja'
$env:CMAKE_LINKER_TYPE = 'LLD'
$env:CMAKE_POLICY_VERSION_MINIMUM = '3.5'
$env:CXXFLAGS = '/EHsc'; $env:CL = '/EHsc'
if (Test-Path C:\VulkanSDK) {
    $env:VULKAN_SDK = (Get-ChildItem C:\VulkanSDK -Directory |
        Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1).FullName
}
foreach ($v in 'VSINSTALLDIR', 'CMAKE_GENERATOR_INSTANCE', 'CMAKE_GENERATOR_PLATFORM', 'CMAKE_GENERATOR_TOOLSET') {
    Remove-Item "env:$v" -ErrorAction SilentlyContinue
}
Write-Host "echo dev env ready (VULKAN_SDK=$env:VULKAN_SDK)" -ForegroundColor DarkGray
