param(
    [ValidateSet("fast", "release")]
    [string]$Profile = "fast",
    [switch]$NoDiarization,
    [switch]$Bundle
)

$ErrorActionPreference = "Stop"

Write-Host "--- Echo: Fast Build Workflow ---" -ForegroundColor Cyan

# 1. Autodetect Vulkan SDK
$vulkanPath = "C:\VulkanSDK"
if (Test-Path $vulkanPath) {
    $latestSdk = Get-ChildItem $vulkanPath -Directory | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    if ($null -ne $latestSdk) {
        $env:VULKAN_SDK = $latestSdk.FullName
        Write-Host "Using VULKAN_SDK: $($env:VULKAN_SDK)"
    }
}

# 2. Find and import VsDevCmd.bat
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $vsPath = & $vswhere -latest -property installationPath
    if ($vsPath) {
        $vsDevCmd = Join-Path $vsPath "Common7\Tools\VsDevCmd.bat"
        if (Test-Path $vsDevCmd) {
            Write-Host "Importing MSVC environment (x64)..."
            $tempFile = [IO.Path]::GetTempFileName()
            cmd.exe /c "call `"$vsDevCmd`" -arch=x64 > nul && set > `"$tempFile`""
            Get-Content $tempFile | ForEach-Object {
                # [^=]+ (not .*?): cmd emits hidden vars like "=C:=..." whose
                # empty name would crash SetEnvironmentVariable under -Stop.
                if ($_ -match "^([^=]+)=(.*)$") {
                    $name = $Matches[1]
                    $value = $Matches[2]
                    # Avoid overwriting critical PowerShell/System variables
                    if ($name -notmatch "^(ALLUSERSPROFILE|APPDATA|COMPUTERNAME|ComSpec|CommonProgramFiles|CommonProgramW64|ConfigSetRoot|DriverData|HOMEDRIVE|HOMEPATH|LOCALAPPDATA|LOGONSERVER|NUMBER_OF_PROCESSORS|OS|PATHEXT|PROCESSOR_ARCHITECTURE|PROCESSOR_IDENTIFIER|PROCESSOR_LEVEL|PROCESSOR_REVISION|ProgramData|ProgramFiles|ProgramW64|PSModulePath|PUBLIC|SystemDrive|SystemRoot|TEMP|TMP|USERDOMAIN|USERDOMAIN_ROAMING_PROFILE|USERNAME|USERPROFILE|windir)$") {
                        [Environment]::SetEnvironmentVariable($name, $value, "Process")
                    }
                }
            }
            Remove-Item $tempFile
        }
    }
}

# 3. Setup LLVM/Ninja Environment
$env:CC = "clang-cl"
$env:CXX = "clang-cl"
$env:CMAKE_GENERATOR = "Ninja"
$env:CMAKE_LINKER_TYPE = "LLD"
$env:CMAKE_POLICY_VERSION_MINIMUM = "3.5"
$env:CXXFLAGS = "/EHsc"
$env:CL = "/EHsc"

# Clear VS instance vars that break Ninja
$vsVars = @("VSINSTALLDIR", "CMAKE_GENERATOR_INSTANCE", "CMAKE_GENERATOR_PLATFORM", "CMAKE_GENERATOR_TOOLSET")
foreach ($var in $vsVars) {
    Remove-Item "env:$var" -ErrorAction SilentlyContinue
}

# 4. Construct Command
if ($Bundle) {
    # tauri-cli appends --release itself; forwarding a custom --profile would
    # conflict and the bundler looks for artifacts in target/release anyway.
    # Bundles always build with the shipping `release` profile.
    $tauriArgs = @()
    if ($NoDiarization) { $tauriArgs = @("--", "--", "--no-default-features") }
    Write-Host "Executing: npm run tauri build $($tauriArgs -join ' ')" -ForegroundColor Green
    npm run tauri build @tauriArgs
} else {
    $cargoArgs = @("--profile", $Profile)
    if ($NoDiarization) { $cargoArgs += "--no-default-features" }
    $manifest = Join-Path $PSScriptRoot "..\src-tauri\Cargo.toml"
    Write-Host "Executing: cargo build --manifest-path $manifest $($cargoArgs -join ' ')" -ForegroundColor Green
    cargo build --manifest-path $manifest @cargoArgs
}
exit $LASTEXITCODE
