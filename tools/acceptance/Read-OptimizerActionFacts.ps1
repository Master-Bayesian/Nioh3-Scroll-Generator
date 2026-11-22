#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('Ninja','Samurai')][string]$Form,
    [Parameter(Mandatory)][string]$Reader,
    [Parameter(Mandatory)][string]$DataRoot,
    [Parameter(Mandatory)][string]$Save,
    [Parameter(Mandatory)][string]$Output,
    [string]$Scene,
    [string]$Fixture
)
$ErrorActionPreference='Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
$run=(New-Item -ItemType Directory -Path $Output).FullName
$copy=Join-Path $run 'SAVEDATA.BIN'
Copy-Item -LiteralPath $Save -Destination $copy
if ((Get-FileHash -LiteralPath $copy).Hash -ne (Get-FileHash -LiteralPath $Save).Hash) {
    throw 'The save changed while it was being copied.'
}
if (!$Scene) { $Scene=if($Form -eq 'Ninja'){'kusarigama-strong'}else{'samurai-dual-swords-mid-quick'} }
$opts=@('--scene',$scene,'--action-facts','record')
if($Fixture){$opts+=@('--fixture',$Fixture)}
& $Reader $DataRoot $copy "$run/read" @opts
if($LASTEXITCODE){throw "Role export failed; inspect $run/read."}
$facts=Get-Content "$run/read/action-facts.json" -Raw|ConvertFrom-Json
$f=$facts.last.fields
if(!$facts.unchanged){throw "Values changed between the two reads; both were retained in $run/read."}
$required=@('physical_and_elemental_factors','status_element')
if($null -ne $f.status_element.bytes -and $f.status_element.bytes -ne 'ffffffff'){$required+='status_element_power'}
if($Form -eq 'Samurai'){
    $required+=@('stance_ki_factors','descriptor','agility_descriptor_flags','control')
    if($null -ne $f.control.bytes -and $f.control.bytes -ne '0000000000000000'){
        $required+='control_mask'
        if($null -ne $f.control_mask.bytes -and $f.control_mask.bytes -ne '0000000000000000'){$required+='cost_modifier_exclusion'}
    }
}
$missing=@($required | Where-Object {$null -eq $f.$_.bytes})
$receipt=[ordered]@{form=$Form; source=$facts.source; scene=$scene; pid=$facts.pid;
    process_creation=$facts.creation_filetime; unchanged=$facts.unchanged;
    required=$required; missing=$missing; complete=($missing.Count -eq 0); read="$run/read"}
$effectPath=Join-Path $run 'read/effect-query-inputs.json'
if(Test-Path -LiteralPath $effectPath -PathType Leaf){
    $effect=Get-Content -LiteralPath $effectPath -Raw | ConvertFrom-Json
    $bound=$effect.pid -eq $facts.pid -and $effect.creation_filetime -eq $facts.creation_filetime -and
        $effect.first.owner_before -eq $effect.first.owner -and $effect.first.owner_after -eq $effect.first.owner -and
        $effect.last.owner_before -eq $effect.last.owner -and $effect.last.owner_after -eq $effect.last.owner
    $scope=if(!$bound){'role_changed'}elseif(!$effect.unchanged){'changed'}else{$effect.last.query.state}
    $receipt.effect_object_lookup=[ordered]@{state=$scope; source=$effect.source;
        additive_bytes=$effect.last.conditional_packet_additive; report=$effectPath}
}
$receipt|ConvertTo-Json -Depth 5|Set-Content "$run/ACTION_READ.json" -Encoding utf8NoBOM
$receipt|ConvertTo-Json -Depth 5
if($missing.Count){throw "Missing action inputs: $($missing -join ', ')"}
