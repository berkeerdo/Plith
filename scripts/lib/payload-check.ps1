# scripts/lib/payload-check.ps1: proves the installer really embeds the signed payload.
# Dot-source it.

Add-Type -TypeDefinition @'
public static class PlithBytes {
    // Index of the first occurrence of needle in haystack, or -1.
    public static long IndexOf(byte[] haystack, byte[] needle) {
        if (needle.Length == 0) return 0;
        int last = haystack.Length - needle.Length;
        byte first = needle[0];
        for (int i = System.Array.IndexOf(haystack, first, 0); i >= 0 && i <= last;
             i = System.Array.IndexOf(haystack, first, i + 1)) {
            int j = 1;
            while (j < needle.Length && haystack[i + j] == needle[j]) j++;
            if (j == needle.Length) return i;
        }
        return -1;
    }
}
'@

# Throws unless every payload file is a byte-identical copy of its signed original, is Valid
# with the pinned thumbprint, and appears in full inside the setup exe.
function Assert-PlithPayloadEmbedded {
    param(
        [string]$SetupExe,
        [string]$PayloadDir,
        [hashtable]$Originals,  # payload file name -> path of the signed original
        [string]$PinnedThumbprint
    )
    $setupBytes = [System.IO.File]::ReadAllBytes($SetupExe)
    foreach ($name in $Originals.Keys) {
        $copy = Join-Path $PayloadDir $name
        $original = $Originals[$name]
        $copyHash = (Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash
        $originalHash = (Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash
        if ($copyHash -ne $originalHash) { throw "Payload copy $copy differs from the signed $original." }
        $sig = Get-AuthenticodeSignature -LiteralPath $copy
        if ($sig.Status -ne 'Valid') { throw "Payload $copy signature is $($sig.Status), expected Valid." }
        if ($sig.SignerCertificate.Thumbprint -ne $PinnedThumbprint) {
            throw "Payload $copy is signed by $($sig.SignerCertificate.Thumbprint), not the pinned certificate."
        }
        $index = [PlithBytes]::IndexOf($setupBytes, [System.IO.File]::ReadAllBytes($copy))
        if ($index -lt 0) { throw "$SetupExe does not contain the full signed ${name}: the payload was not embedded." }
        Write-Host "Embedded $name at offset $index."
    }
}
