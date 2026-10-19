# Offline Windows export of our path-only SVG. No app/build-time dependency.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName PresentationCore, WindowsBase
$projectRoot = Split-Path $PSScriptRoot -Parent
$assetRoot = Join-Path $projectRoot 'assets'
[xml]$svg = Get-Content -LiteralPath (Join-Path $assetRoot 'mem-map.svg') -Raw
if ($svg.svg.viewBox -ne '0 0 256 256') { throw 'Expected a 256-unit icon canvas' }
foreach ($node in $svg.svg.ChildNodes) {
    if ($node.NodeType -eq 'Element' -and $node.LocalName -notin @('title', 'desc', 'path')) {
        throw "The icon exporter supports path artwork only: $($node.LocalName)"
    }
}
$drawing = [Windows.Media.DrawingGroup]::new()
foreach ($path in $svg.svg.path) {
    foreach ($attribute in $path.Attributes) {
        if ($attribute.LocalName -notin @('d', 'fill', 'stroke', 'stroke-width')) {
            throw "Unsupported icon path attribute: $($attribute.LocalName)"
        }
    }
    $brush = [Windows.Media.BrushConverter]::new().ConvertFromInvariantString($path.fill)
    $pen = $null
    if ($path.stroke) {
        $stroke = [Windows.Media.BrushConverter]::new().ConvertFromInvariantString($path.stroke)
        $pen = [Windows.Media.Pen]::new($stroke, [double]$path.'stroke-width')
    }
    $drawing.Children.Add([Windows.Media.GeometryDrawing]::new($brush, $pen, [Windows.Media.Geometry]::Parse('F1 ' + $path.d)))
}
$entries = foreach ($size in @(16, 20, 24, 32, 40, 48, 64, 128, 256)) {
    $visual = [Windows.Media.DrawingVisual]::new()
    $context = $visual.RenderOpen()
    $context.PushTransform([Windows.Media.ScaleTransform]::new($size / 256.0, $size / 256.0))
    $context.DrawDrawing($drawing)
    $context.Pop()
    $context.Close()
    $bitmap = [Windows.Media.Imaging.RenderTargetBitmap]::new($size, $size, 96, 96, [Windows.Media.PixelFormats]::Pbgra32)
    $bitmap.Render($visual)
    $encoder = [Windows.Media.Imaging.PngBitmapEncoder]::new()
    $encoder.Frames.Add([Windows.Media.Imaging.BitmapFrame]::Create($bitmap))
    $stream = [IO.MemoryStream]::new()
    try {
        $encoder.Save($stream)
        [pscustomobject]@{ Size = $size; Bytes = $stream.ToArray() }
    } finally { $stream.Dispose() }
}
[IO.File]::WriteAllBytes((Join-Path $assetRoot 'mem-map.png'), $entries[-1].Bytes)
$file = [IO.File]::Create((Join-Path $assetRoot 'mem-map.ico'))
$writer = [IO.BinaryWriter]::new($file)
try {
    $writer.Write([uint16]0)
    $writer.Write([uint16]1)
    $writer.Write([uint16]$entries.Count)
    $offset = 6 + 16 * $entries.Count
    foreach ($entry in $entries) {
        $dimension = if ($entry.Size -eq 256) { 0 } else { $entry.Size }
        $writer.Write([byte]$dimension)
        $writer.Write([byte]$dimension)
        $writer.Write([uint16]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]32)
        $writer.Write([uint32]$entry.Bytes.Length)
        $writer.Write([uint32]$offset)
        $offset += $entry.Bytes.Length
    }
    foreach ($entry in $entries) { $writer.Write([byte[]]$entry.Bytes) }
} finally { $writer.Dispose() }
Get-Item -LiteralPath (Join-Path $assetRoot 'mem-map.ico'), (Join-Path $assetRoot 'mem-map.png') | Select-Object Name, Length
