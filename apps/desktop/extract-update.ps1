param([Parameter(Mandatory)][string]$Archive,[Parameter(Mandatory)][string]$Destination)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$target=[IO.Path]::GetFullPath($Destination)
if(Test-Path -LiteralPath $target){throw 'Destination already exists'}
$zip=[IO.Compression.ZipFile]::OpenRead($Archive)
try {
    $total=0L
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    if($zip.Entries.Count -gt 15000){throw 'Too many archive entries'}
    foreach($entry in $zip.Entries){
        $name=$entry.FullName.Replace('\','/')
        if($name -match '(^/|:|(^|/)\.\.?(/|$))' -or (($entry.ExternalAttributes -shr 16) -band 0xF000) -eq 0xA000){throw 'Unsafe archive entry'}
        $path=[IO.Path]::GetFullPath([IO.Path]::Combine($target,$name))
        if(-not $path.StartsWith($target+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Archive escapes destination'}
        if(-not $seen.Add($path)){throw 'Duplicate archive entry'}
        $total+=$entry.Length
        if($total -gt 3GB){throw 'Unpacked update is too large'}
    }
    [IO.Directory]::CreateDirectory($target)|Out-Null
    foreach($entry in $zip.Entries){
        $path=[IO.Path]::GetFullPath([IO.Path]::Combine($target,$entry.FullName))
        if($entry.FullName.EndsWith('/')){[IO.Directory]::CreateDirectory($path)|Out-Null;continue}
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($path))|Out-Null
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$path,$false)
    }
} finally {$zip.Dispose()}
