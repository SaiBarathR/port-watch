# Redirected stdout defaults to the OEM codepage, which mangles non-ASCII
# user names and paths — force UTF-8 so Rust can parse it losslessly.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
# One process snapshot instead of one WMI query per connection.
$procs = @{}
foreach ($p in (Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)) {
  $procs[[uint32]$p.ProcessId] = $p
}
$result = @()
$connections = Get-NetUDPEndpoint -ErrorAction SilentlyContinue
foreach ($conn in $connections) {
  $proc = $procs[[uint32]$conn.OwningProcess]
  if ($null -eq $proc) { continue }
  $owner = $proc | Invoke-CimMethod -MethodName GetOwner -ErrorAction SilentlyContinue
  $user = if ($owner -and $owner.User) { "$($owner.Domain)\$($owner.User)" } else { "" }
  $uptime = 0
  if ($proc.CreationDate) {
    $created = [Management.ManagementDateTimeConverter]::ToDateTime($proc.CreationDate)
    $uptime = [Math]::Max(0, [int]([DateTime]::UtcNow - $created.ToUniversalTime()).TotalSeconds)
  }
  $result += [PSCustomObject]@{
    pid = [int]$conn.OwningProcess
    name = [string]$proc.Name
    user = [string]$user
    localAddress = [string]$conn.LocalAddress
    localPort = [int]$conn.LocalPort
    executablePath = [string]$proc.ExecutablePath
    commandLine = [string]$proc.CommandLine
    protocol = "UDP"
    uptimeSeconds = $uptime
  }
}
if ($result.Count -eq 0) { "" } else { ConvertTo-Json -InputObject @($result) -Compress -Depth 4 }
