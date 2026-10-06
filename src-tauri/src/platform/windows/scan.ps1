# Redirected stdout defaults to the OEM codepage, which mangles non-ASCII
# user names and paths — force UTF-8 so Rust can parse it losslessly.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$includeUdp = __INCLUDE_UDP__

$sockets = New-Object System.Collections.Generic.List[object]
foreach ($c in @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue)) {
  $sockets.Add(@{ owner = [uint32]$c.OwningProcess; address = [string]$c.LocalAddress; port = [int]$c.LocalPort; protocol = "TCP" })
}
if ($includeUdp) {
  foreach ($c in @(Get-NetUDPEndpoint -ErrorAction SilentlyContinue)) {
    $sockets.Add(@{ owner = [uint32]$c.OwningProcess; address = [string]$c.LocalAddress; port = [int]$c.LocalPort; protocol = "UDP" })
  }
}

$wanted = @{}
foreach ($s in $sockets) { $wanted[$s.owner] = $true }

# One process snapshot, and the owner looked up once per process rather than
# once per socket.
$processes = @{}
foreach ($p in @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)) {
  $id = [uint32]$p.ProcessId
  if (-not $wanted.ContainsKey($id)) { continue }

  $owner = $p | Invoke-CimMethod -MethodName GetOwner -ErrorAction SilentlyContinue
  $user = if ($owner -and $owner.User) { "$($owner.Domain)\$($owner.User)" } else { "" }

  # CIM hands back a DateTime; WMI proper would hand back a DMTF string.
  $startedAt = 0
  $created = $p.CreationDate
  if ($created -is [string]) {
    $created = [Management.ManagementDateTimeConverter]::ToDateTime($created)
  }
  if ($created -is [DateTime]) {
    $startedAt = (New-Object System.DateTimeOffset -ArgumentList $created).ToUnixTimeSeconds()
  }

  $processes[$id] = @{
    name = [string]$p.Name
    user = [string]$user
    executablePath = [string]$p.ExecutablePath
    commandLine = [string]$p.CommandLine
    startedAt = [long]$startedAt
  }
}

$result = New-Object System.Collections.Generic.List[object]
foreach ($s in $sockets) {
  $p = $processes[$s.owner]
  if ($null -eq $p) { continue }
  $result.Add([PSCustomObject]@{
    pid = [int]$s.owner
    name = $p.name
    user = $p.user
    localAddress = $s.address
    localPort = $s.port
    executablePath = $p.executablePath
    commandLine = $p.commandLine
    protocol = $s.protocol
    startedAt = $p.startedAt
  })
}
if ($result.Count -eq 0) { "" } else { ConvertTo-Json -InputObject $result.ToArray() -Compress -Depth 4 }
