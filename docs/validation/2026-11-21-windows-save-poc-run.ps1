$ErrorActionPreference='Stop'
Add-Type -Path (Join-Path $PSScriptRoot 'save-probe.cs')
$dir=Get-Content (Join-Path $PSScriptRoot 'save-poc-root.txt')
$probe=[SaveProbe]::new($dir,(Join-Path $dir 'events.jsonl'))
function Snapshot($phase,$path){[ordered]@{phase=$phase;utc=(Get-Date).ToUniversalTime().ToString('o');path=$path;file_id=@(& fsutil file queryfileid $path);object_id=@(& fsutil objectid query $path);object_exit=$LASTEXITCODE;length=(Get-Item -LiteralPath $path).Length} | ConvertTo-Json -Compress | Add-Content (Join-Path $dir 'snapshots.jsonl')}
try{
$office=New-Object -ComObject Word.Application
$office.Visible=$true
foreach($name in @('control','objectid')){
$path=Join-Path $dir ($name+'.docx')
Snapshot "$name-before-open" $path
$probe.Mark("$name-open")
$doc=$office.Documents.Open($path)
foreach($n in 1..3){$doc.Content.Text="WORKTWILL_POC_${name}_SAVE_$n";$probe.Mark("$name-save-$n-begin");$doc.Save();Start-Sleep -Milliseconds 350;Snapshot "$name-save-$n" $path;$probe.Mark("$name-save-$n-end")}
$doc.Close(0);[System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($doc) | Out-Null
Snapshot "$name-after-close" $path
}
[System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($office) | Out-Null
$target=Join-Path $dir 'adversarial.docx';Copy-Item (Join-Path $dir 'control.docx') $target
Snapshot 'adversarial-before' $target
$probe.Mark('adversarial-equivalent-rename-chain')
$temp=Join-Path $dir 'replacement.tmp';Copy-Item (Join-Path $dir 'objectid.docx') $temp
Move-Item -LiteralPath $target -Destination (Join-Path $dir 'old-backup.tmp')
Move-Item -LiteralPath $temp -Destination $target
Remove-Item -LiteralPath (Join-Path $dir 'old-backup.tmp')
Start-Sleep -Milliseconds 350
Snapshot 'adversarial-after' $target
}finally{$probe.Dispose()}
