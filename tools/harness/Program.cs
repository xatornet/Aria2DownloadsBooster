// Arnés de prueba del nivel de servicios: replica la secuencia exacta de la app
// (arrancar motor → añadir descarga A → añadir descarga B → sondear con multicall)
// en un APPDATA aislado, sin tocar los datos reales de Aria-XTRDownloader.
using System.Diagnostics;
using System.Text.Json;
using AriaXtr.Services;

// Datos aislados vía ARIA_XTR_DATA (lee la ruta REAL antes, sin cachear GetFolderPath)
var realAppData = Environment.GetEnvironmentVariable("APPDATA")
                  ?? throw new InvalidOperationException("APPDATA no definido");
var tempAppData = Path.Combine(Path.GetTempPath(), "ariaxtr-harness-" + Guid.NewGuid().ToString("N")[..8]);
Environment.SetEnvironmentVariable("ARIA_XTR_DATA", tempAppData);

// Copiar el binario aria2c ya instalado por la app
var realEngine = Path.Combine(realAppData, "Aria-XTRDownloader", "engine", "aria2c.exe");

AppPaths.EnsureBase();
if (File.Exists(realEngine))
{
    // El daemon de la app real puede tener el exe abierto: copiar compartido
    using var src = new FileStream(realEngine, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
    using var dst = File.Create(AppPaths.EngineBinary);
    src.CopyTo(dst);
}
else
{
    Console.WriteLine("SIN BINARIO aria2c — prueba abortada");
    return 2;
}

var eng = new EngineService();
var ok = await eng.EnsureStartedAsync(new Settings());
Console.WriteLine($"motor arrancado={ok} estado={eng.State} version={eng.Version} err={eng.LastError}");
if (!ok || eng.Rpc == null) return 1;

var rpc = eng.Rpc;
var dir = Path.Combine(tempAppData, "dl");
Directory.CreateDirectory(dir);

var urls = new[]
{
    "https://github.com/aria2/aria2/releases/download/release-1.37.0/aria2-1.37.0.tar.xz",
    "https://github.com/aria2/aria2/releases/download/release-1.36.0/aria2-1.36.0.tar.xz",
};

var gids = new List<string>();
for (int i = 0; i < urls.Length; i++)
{
    try
    {
        var opts = new Dictionary<string, string>
        {
            ["dir"] = dir,
            ["out"] = $"dl{i}.bin",
            ["continue"] = "true",
            ["maxConnectionPerServer"] = "16",
            ["split"] = "16",
            ["autoFileRenaming"] = "false",
            ["allowOverwrite"] = "true",
        };
        var gid = (await rpc.CallAsync("aria2.addUri",
            new object?[] { new List<string> { urls[i] }, opts })).GetString();
        gids.Add(gid!);
        Console.WriteLine($"ADD #{i + 1} OK gid={gid}");
    }
    catch (Exception ex)
    {
        Console.WriteLine($"ADD #{i + 1} FALLO: {ex.Message}");
    }
}

await Task.Delay(4000);

// Sondeo idéntico al de la app: tellStatus×N + tellActive + tellWaiting
var fields = new[] { "status", "totalLength", "completedLength", "downloadSpeed", "connections", "errorMessage" };
var calls = new List<(string Method, object?[] Args)>(
    gids.Select(g => ("aria2.tellStatus", new object?[] { g, fields })));
calls.Add(("aria2.tellActive", Array.Empty<object?>()));

var res = await rpc.MulticallAsync(calls);
var elems = res.EnumerateArray().ToArray();
Console.WriteLine($"MULTICALL elems={elems.Length}");
for (int i = 0; i < gids.Count && i < elems.Length - 1; i++)
{
    if (MulticallResult.TryGet(elems[i], out var st))
    {
        Console.WriteLine($"  [{i}] status={st.GetProperty("status")} total={st.GetProperty("totalLength")} " +
                          $"done={st.GetProperty("completedLength")} speed={st.GetProperty("downloadSpeed")} " +
                          $"conns={st.GetProperty("connections")}");
    }
    else
    {
        Console.WriteLine($"  [{i}] ERROR: {MulticallResult.GetError(elems[i])}");
    }
}
Console.WriteLine($"  tellActive={(CountArr(elems[^1]) >= 0 ? CountArr(elems[^1]) : -1)}");

// === Prueba de recuperación tras reinicio del demonio (bug reportado) ===
Console.WriteLine("== recuperación tras reinicio del demonio ==");
const string big = "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.12.tar.xz";
var optsBig = new Dictionary<string, string>
{
    ["dir"] = dir, ["out"] = "big.bin", ["continue"] = "true",
    ["maxConnectionPerServer"] = "8", ["split"] = "8",
    ["autoFileRenaming"] = "false", ["allowOverwrite"] = "true",
};
var gidBig = (await rpc.CallAsync("aria2.addUri",
    new object?[] { new List<string> { big }, optsBig })).GetString()!;
await Task.Delay(3000); // que descargue algo
var stB = await rpc.CallAsync("aria2.tellStatus", new object?[] { gidBig, fields });
long doneBefore = long.Parse(stB.GetProperty("completedLength").GetString()!);
Console.WriteLine($"  gran descarga activa con {doneBefore} bytes (se corta el demonio sin pausar)");

// "muere" el demonio (cierre de app) y se re-arranca vacío
await eng.StopDaemonForShutdownAsync();
var ok2 = await eng.EnsureStartedAsync(new Settings());
rpc = eng.Rpc!;
Console.WriteLine($"  demonio re-arrancado={ok2}");

// el gid viejo debe reportarse como desconocido Y IsUnknownGid debe detectarlo
bool detectOk = false;
try { await rpc.CallAsync("aria2.tellStatus", new object?[] { gidBig, fields }); }
catch (AriaRpcException ex)
{
    detectOk = ex.IsUnknownGid;
    Console.WriteLine($"  gid viejo → \"{ex.Message}\" IsUnknownGid={ex.IsUnknownGid}");
}

// Re-add con continue=true (equivalente a ReaddAsync de la app)
var gid2 = (await rpc.CallAsync("aria2.addUri",
    new object?[] { new List<string> { big }, optsBig })).GetString()!;
await Task.Delay(1500);
var st2 = await rpc.CallAsync("aria2.tellStatus",
    new object?[] { gid2, new[] { "status", "completedLength" } });
long doneAfter = long.Parse(st2.GetProperty("completedLength").GetString()!);
Console.WriteLine($"  re-añadida gid={gid2} status={st2.GetProperty("status")} " +
                  $"antes={doneBefore} ahora={doneAfter}");

await eng.StopDaemonForShutdownAsync();
bool recuperada = detectOk && doneAfter >= doneBefore && doneBefore > 0;
Console.WriteLine(recuperada
    ? "RECUPERACIÓN OK (gid muerto detectado + continúa donde estaba)"
    : "RECUPERACIÓN FALLÓ");
Console.WriteLine("PRUEBA COMPLETA");
return recuperada ? 0 : 1;

static int CountArr(JsonElement el) =>
    MulticallResult.TryGet(el, out var v) && v.ValueKind == JsonValueKind.Array ? v.GetArrayLength() : -1;
