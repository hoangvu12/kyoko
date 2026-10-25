param(
	[string]$Path = (Join-Path (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))) "oxidcord-ref"),
	[switch]$Force,
	[switch]$Release,
	[switch]$Export
)

$ErrorActionPreference = "Stop"
$Repository = "https://github.com/Master0fFate/oxidcord"
$Commit = "b852253"
$Patch = Join-Path $PSScriptRoot "oxidcord-ref.patch"

function Invoke-Checked {
	param([string]$What, [scriptblock]$Command)
	& $Command
	if ($LASTEXITCODE -ne 0) {
		throw "$What failed with exit code $LASTEXITCODE"
	}
}

if ($Export) {
	if (-not (Test-Path $Path)) {
		throw "$Path does not exist"
	}
	Invoke-Checked "staging the reference changes" { git -C $Path add -A }
	Invoke-Checked "exporting the patch" { git -C $Path diff --cached --output=$Patch $Commit }
	Write-Host "wrote $Patch"
	return
}

if (Test-Path $Path) {
	if (-not $Force) {
		throw "$Path already exists; pass -Force to replace it"
	}
	Remove-Item -Recurse -Force $Path
}

Invoke-Checked "cloning oxidcord" { git clone --quiet $Repository $Path }
Invoke-Checked "checking out $Commit" { git -C $Path -c advice.detachedHead=false checkout --quiet $Commit }
Invoke-Checked "applying the capture patch" { git -C $Path apply --whitespace=nowarn $Patch }

$Arguments = @("build", "-p", "oxidcord")
if ($Release) {
	$Arguments += "--release"
}
Push-Location $Path
try {
	Invoke-Checked "building oxidcord" { cargo @Arguments }
} finally {
	Pop-Location
}

$BuildProfile = if ($Release) { "release" } else { "debug" }
Write-Host "built $(Join-Path $Path "target\$BuildProfile\oxidcord.exe")"
