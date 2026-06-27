$result = @()
$connections = Get-NetUDPEndpoint -ErrorAction SilentlyContinue
foreach ($conn in $connections) {
  $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$($conn.OwningProcess)" -ErrorAction SilentlyContinue
  if ($null -eq $proc) { continue }
  $owner = $proc | Invoke-CimMethod -MethodName GetOwner -ErrorAction SilentlyContinue
  $user = if ($owner -and $owner.User) { "$($owner.Domain)\$($owner.User)" } else { "" }
  $uptime = 0
  if ($proc.CreationDate) {
    $created = [Management.ManagementDateTimeConverter]::ToDateTime($proc.CreationDate)
    $uptime = [int]([DateTime]::UtcNow - $created.ToUniversalTime()).TotalSeconds
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
if ($result.Count -eq 0) { "" } else { $result | ConvertTo-Json -Compress -Depth 4 }
