# tools/make-icon.ps1 — genera Assets/icon.ico (multi-resolucion, PNG dentro de ICO).
# Diseno: loseta azul marino con degradado y rayo azul-cian (mismo motivo que la bandeja).
# Uso:  powershell -ExecutionPolicy Bypass -File tools\make-icon.ps1
Add-Type -AssemblyName System.Drawing

function Draw-Icon([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)
    $s = $size / 64.0

    # loseta: rectangulo redondeado con degradado diagonal azul marino -> casi negro
    $x = [single](1.0 * $s); $y = [single](1.0 * $s)
    $w = [single](62.0 * $s); $h = [single](62.0 * $s)
    $r = [single](14.0 * $s); $d = $r * 2
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc($x, $y, $d, $d, 180, 90)
    $path.AddArc(($x + $w - $d), $y, $d, $d, 270, 90)
    $path.AddArc(($x + $w - $d), ($y + $h - $d), $d, $d, 0, 90)
    $path.AddArc($x, ($y + $h - $d), $d, $d, 90, 90)
    $path.CloseFigure()

    $tileRect = New-Object System.Drawing.RectangleF(0, 0, [single]$size, [single]$size)
    $tile = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $tileRect,
        [System.Drawing.Color]::FromArgb(255, 34, 45, 88),
        [System.Drawing.Color]::FromArgb(255, 12, 15, 30),
        60.0)
    $g.FillPath($tile, $path)

    # borde sutil de la loseta
    $edge = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(90, 96, 130, 220), [single](max 1.0 (0.9 * $s)))
    $g.DrawPath($edge, $path)

    # rayo: degradado azul -> cian
    $pts = @(
        [System.Drawing.PointF]::new([single](40 * $s), [single](4 * $s)),
        [System.Drawing.PointF]::new([single](18 * $s), [single](36 * $s)),
        [System.Drawing.PointF]::new([single](29 * $s), [single](36 * $s)),
        [System.Drawing.PointF]::new([single](25 * $s), [single](58 * $s)),
        [System.Drawing.PointF]::new([single](48 * $s), [single](26 * $s)),
        [System.Drawing.PointF]::new([single](34 * $s), [single](26 * $s))
    )
    $boltRect = New-Object System.Drawing.RectangleF([single](18 * $s), [single](4 * $s), [single](30 * $s), [single](54 * $s))
    $bolt = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $boltRect,
        [System.Drawing.Color]::FromArgb(255, 139, 185, 255),
        [System.Drawing.Color]::FromArgb(255, 34, 211, 238),
        45.0)
    $g.FillPolygon($bolt, $pts)

    $g.Dispose()
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    return , $ms.ToArray()
}

function Max([double]$a, [double]$b) { if ($a -gt $b) { $a } else { $b } }

$sizes = 16, 24, 32, 48, 64, 128, 256
$pngs = New-Object System.Collections.Generic.List[byte[]]
foreach ($sz in $sizes) { $pngs.Add((Draw-Icon $sz)) }

$out = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter($out)
$bw.Write([uint16]0)            # reserved
$bw.Write([uint16]1)            # type: icon
$bw.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $sz = $sizes[$i]
    $b = if ($sz -ge 256) { [byte]0 } else { [byte]$sz }
    $bw.Write($b)               # width  (0 = 256)
    $bw.Write($b)               # height (0 = 256)
    $bw.Write([byte]0)          # colors
    $bw.Write([byte]0)          # reserved
    $bw.Write([uint16]1)        # planes
    $bw.Write([uint16]32)       # bpp
    $bw.Write([uint32]$pngs[$i].Length)
    $bw.Write([uint32]$offset)
    $offset += $pngs[$i].Length
}
foreach ($p in $pngs) { $bw.Write($p) }
$bw.Flush()

$dst = Join-Path (Split-Path $PSScriptRoot -Parent) 'Assets\icon.ico'
[System.IO.File]::WriteAllBytes($dst, $out.ToArray())
Write-Output "escrito $dst ($((Get-Item $dst).Length) bytes)"
