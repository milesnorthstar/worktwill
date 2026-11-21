$ErrorActionPreference='Stop'
Add-Type -Path (Join-Path $PSScriptRoot 'save-probe.cs')
$dir=Get-Content (Join-Path $PSScriptRoot 'save-poc-root.txt')
function Snapshot($phase,$path){[ordered]@{phase=$phase;utc=(Get-Date).ToUniversalTime().ToString('o');path=$path;file_id=@(& fsutil file queryfileid $path);object_id=@(& fsutil objectid query $path);object_exit=$LASTEXITCODE;length=(Get-Item -LiteralPath $path).Length} | ConvertTo-Json -Compress | Add-Content (Join-Path $dir 'snapshots-followup.jsonl')}
$probe=[SaveProbe]::new($dir,(Join-Path $dir 'events-followup.jsonl'))
try{
$original=Join-Path $dir 'objectid.docx'
$copy=Join-Path $dir 'copy.docx';Copy-Item -LiteralPath $original -Destination $copy;Snapshot 'copy' $copy;Snapshot 'original-after-copy' $original
$office=New-Object -ComObject Word.Application
$doc=$office.Documents.Open($original)
$doc.SaveAs2((Join-Path $dir 'save-as.docx'),16)
Snapshot 'save-as-copy' (Join-Path $dir 'save-as.docx');Snapshot 'original-after-save-as' $original
$doc.Close(0);[System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($doc) | Out-Null
$moved=Join-Path $dir 'moved.docx';Move-Item -LiteralPath $original -Destination $moved;Snapshot 'same-volume-moved' $moved
$probe.Dispose();$probe=$null
$doc=$office.Documents.Open($moved);$doc.Content.Text='WORKTWILL_POC_OBSERVER_STOPPED_SAVE';$doc.Save();$doc.Close(0);[System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($doc) | Out-Null
Snapshot 'observer-stopped-save' $moved
[System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($office) | Out-Null
# 无关DOCX用同一替换API覆盖，检验Object ID是否等于语义连续性。
$victim=Join-Path $dir 'replacefile-victim.docx';Copy-Item -LiteralPath $copy -Destination $victim
& fsutil objectid create $victim | Out-Null
Snapshot 'replacefile-unrelated-before' $victim
$replacement=Join-Path $dir 'unrelated-replacement.docx';Copy-Item -LiteralPath (Join-Path $dir 'control.docx') -Destination $replacement
[System.IO.File]::Replace($replacement,$victim,$null)
Snapshot 'replacefile-unrelated-after' $victim
}finally{if($probe){$probe.Dispose()}}
