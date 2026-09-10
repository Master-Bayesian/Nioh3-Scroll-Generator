$ErrorActionPreference='Stop'
# Resolve before tests isolate LOCALAPPDATA for synthetic saves.
function RuntimeFolder {
    $roots=@((Join-Path ${env:ProgramFiles(x86)} 'Microsoft/EdgeWebView/Application'),(Join-Path $env:LOCALAPPDATA 'Microsoft/EdgeWebView/Application'))
    foreach($root in $roots){
        if(Test-Path -LiteralPath $root){
            $match=Get-ChildItem -LiteralPath $root -Directory | Where-Object {Test-Path -LiteralPath (Join-Path $_.FullName 'msedgewebview2.exe')} | Sort-Object {[version]$_.Name} -Descending | Select-Object -First 1
            if($match){return $match.FullName}
        }
    }
}
$folder=RuntimeFolder
if(-not $folder){
    $bootstrap=Join-Path $env:RUNNER_TEMP 'MicrosoftEdgeWebview2Setup.exe'
    Invoke-WebRequest 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $bootstrap
    $signature=Get-AuthenticodeSignature -LiteralPath $bootstrap
    if($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation'){throw 'WebView2 bootstrapper signature invalid'}
    Start-Process -FilePath $bootstrap -ArgumentList '/silent','/install' -Wait -WindowStyle Hidden
    for($i=0;$i -lt 60 -and -not $folder;$i++){Start-Sleep -Seconds 2;$folder=RuntimeFolder}
}
if(-not $folder){throw 'WebView2 runtime unavailable before UI tests'}
Write-Output "WebView2 test runtime: $folder"
"WEBVIEW2_BROWSER_EXECUTABLE_FOLDER=$folder" >> $env:GITHUB_ENV
