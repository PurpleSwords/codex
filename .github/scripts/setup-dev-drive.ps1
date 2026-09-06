# Configure a fast drive for Windows CI jobs.
#
# GitHub-hosted Windows runners do not always expose a secondary D: volume. When
# the image supports Dev Drive, create a VHD if necessary. Windows Server 2022
# does not expose Format-Volume's -DevDrive parameter, so use the runner's
# ordinary work volume there instead.

function Test-DevDrive {
    param([string]$Drive)

    & fsutil devdrv query $Drive *> $null
    return $LASTEXITCODE -eq 0
}

function Invoke-BestEffort {
    param([scriptblock]$Script, [string]$Description)

    try {
        & $Script
    } catch {
        Write-Warning "$Description failed: $($_.Exception.Message)"
    }
}

$FormatVolume = Get-Command Format-Volume -ErrorAction Stop
$SupportsDevDrive = $FormatVolume.Parameters.ContainsKey("DevDrive")

if ($SupportsDevDrive -and (Test-Path "D:\") -and (Test-DevDrive "D:")) {
    Write-Output "Using existing Dev Drive at D:"
    $Drive = "D:"
} elseif (-not $SupportsDevDrive) {
    if (Test-Path "D:\") {
        $Drive = "D:"
    } else {
        $Drive = [System.IO.Path]::GetPathRoot($env:RUNNER_TEMP).TrimEnd("\")
    }

    Write-Warning "Dev Drive is unavailable on this Windows image; using the standard volume at $Drive"
} else {
    if (Test-Path "D:\") {
        Write-Output "Existing D: volume is not a Dev Drive; provisioning a new Dev Drive VHD."
    }

    try {
        $VhdPath = Join-Path $env:RUNNER_TEMP "codex-dev-drive.vhdx"
        $SizeBytes = 64GB

        if (Test-Path $VhdPath) {
            Remove-Item -Path $VhdPath -Force
        }

        New-VHD -Path $VhdPath -SizeBytes $SizeBytes -Dynamic -ErrorAction Stop | Out-Null
        $Mounted = Mount-VHD -Path $VhdPath -Passthru -ErrorAction Stop
        $Disk = $Mounted | Get-Disk -ErrorAction Stop
        $Disk | Initialize-Disk -PartitionStyle GPT -ErrorAction Stop
        $Partition = $Disk | New-Partition -AssignDriveLetter -UseMaximumSize -ErrorAction Stop
        $Volume = $Partition | Format-Volume -FileSystem ReFS -NewFileSystemLabel "CodexDevDrive" -DevDrive -Confirm:$false -Force -ErrorAction Stop

        $Drive = "$($Volume.DriveLetter):"

        if (-not (Test-DevDrive $Drive)) {
            throw "Provisioned volume at $Drive did not pass Dev Drive verification."
        }

        Invoke-BestEffort { fsutil devdrv trust $Drive } "Trusting Dev Drive $Drive"
        Invoke-BestEffort { fsutil devdrv enable /disallowAv } "Disabling AV filter attachment for Dev Drives"

        Write-Output "Using Dev Drive at $Drive"
    } catch {
        throw "Failed to create Dev Drive: $($_.Exception.Message)"
    }
}

"CI_BUILD_ROOT=$Drive" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
