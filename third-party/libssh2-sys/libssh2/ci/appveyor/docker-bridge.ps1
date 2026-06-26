#!/usr/bin/env pwsh
# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause

# Partially copied from https://github.com/appveyor/ci/blob/master/scripts/enable-rdp.ps1

# get current IP
$ip = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object {$_.InterfaceAlias -like 'ethernet*'}).IPAddress
$port = 3389
if($ip.StartsWith('172.24.')) {
  $port = 33800 + ($ip.split('.')[2] - 16) * 256 + $ip.split('.')[3]
}
elseif($ip.StartsWith('192.168.') -or $ip.StartsWith('10.240.')) {
  # new environment - behind NAT
  $port = 33800 + ($ip.split('.')[2] - 0) * 256 + $ip.split('.')[3]
}
elseif($ip.StartsWith('10.0.')) {
  $port = 33800 + ($ip.split('.')[2] - 0) * 256 + $ip.split('.')[3]
}

# get external IP
$extip = (New-Object Net.WebClient).DownloadString('https://www.appveyor.com/tools/my-ip.aspx').Trim()

# allow inbound traffic
New-NetFirewallRule -DisplayName 'SSH via RDP port' -Direction Inbound -Action Allow -Protocol TCP -LocalPort 22,3389

# launch remote docker daemon with reverse SSH tunnel
$scriptPath = (split-path -parent $MyInvocation.MyCommand.Definition) -replace '\\', '/'
& C:\msys64\usr\bin\sh -l -c "$scriptPath/docker-bridge.sh $ip $extip $port"
