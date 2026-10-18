using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Net.Http;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;

namespace AriaXtr.Services;

public enum EngineState { Missing, Starting, Running, Error }

/// <summary>
/// Descarga el binario aria2c correcto para el SO actual y gestiona el demonio
/// (arranque con RPC, parada ordenada con save-session, reaparición tras reinicio).
/// </summary>
public sealed class EngineService : IDisposable
{
    public const int RpcPort = 6800;
    private static readonly string Secret = "a2db-" + Guid.NewGuid().ToString("N");

    private Process? _proc;
    private AriaRpc? _rpc;
    private static readonly HttpClient _http = new() { Timeout = TimeSpan.FromMinutes(15) };

    public EngineState State { get; private set; } = EngineState.Missing;
    public string? Version { get; private set; }
    public string? LastError { get; private set; }
    private readonly System.Text.StringBuilder _stderr = new();
    public AriaRpc? Rpc => _rpc;
    public bool IsRunning => State == EngineState.Running && _rpc != null;

    private string LastStderr(int max = 400)
    {
        lock (_stderr)
        {
            var s = _stderr.ToString().Trim();
            return s.Length > max ? "…" + s.Substring(s.Length - max) : s;
        }
    }

    public string BinaryPath => AppPaths.EngineBinary;
    public bool BinaryPresent => File.Exists(BinaryPath);

    /// <summary>URLs candidatas por SO. Se prueban en orden (GitHub oficial primero, mirrors después).</summary>
    public static IEnumerable<string> DownloadUrls =>
        RuntimeInformation.IsOSPlatform(OSPlatform.Windows)
            ? new[]
            {
                "https://github.com/aria2/aria2/releases/download/release-1.37.0/aria2-1.37.0-win-64bit-build1.zip",
                "https://downloads.sourceforge.net/project/aria2.mirror/release-1.37.0/aria2-1.37.0-win-64bit-build1.zip",
                "https://master.dl.sourceforge.net/project/aria2.mirror/release-1.37.0/aria2-1.37.0-win-64bit-build1.zip",
            }
            : new[]
            {
                "https://github.com/P3TERX/Aria2-Pro-Core/releases/download/1.36.0_2021.08.22/aria2-1.36.0-static-linux-amd64.tar.gz",
            };

    /// <summary>
    /// Descarga el motor con progreso. progress(recibido, total|-1). Lanza excepción si falla.
    /// </summary>
    public async Task DownloadEngineAsync(IProgress<(long received, long total)> progress, CancellationToken ct = default)
    {
        AppPaths.EnsureBase();
        var isWin = RuntimeInformation.IsOSPlatform(OSPlatform.Windows);
        var archive = Path.Combine(AppPaths.TempDir, isWin ? "aria2.zip" : "aria2.tar.gz");

        Exception? last = null;
        foreach (var url in DownloadUrls)
        {
            try
            {
                await using var fs = File.Create(archive);
                using var resp = await _http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct);
                resp.EnsureSuccessStatusCode();
                var total = resp.Content.Headers.ContentLength ?? -1;

                await using var src = await resp.Content.ReadAsStreamAsync(ct);
                var buffer = new byte[81920];
                long done = 0;
                int n;
                var lastReport = DateTime.UtcNow;
                while ((n = await src.ReadAsync(buffer, ct)) > 0)
                {
                    await fs.WriteAsync(buffer.AsMemory(0, n), ct);
                    done += n;
                    if (DateTime.UtcNow - lastReport > TimeSpan.FromMilliseconds(150))
                    {
                        progress.Report((done, total));
                        lastReport = DateTime.UtcNow;
                    }
                }
                progress.Report((done, total));
                last = null;
                break;
            }
            catch (Exception ex)
            {
                last = ex;
            }
        }
        if (last != null)
            throw new InvalidOperationException("No se pudo descargar aria2 desde ningún mirror: " + last.Message, last);

        ExtractEngine(archive, isWin);
        try { File.Delete(archive); } catch { /* noop */ }

        if (!File.Exists(BinaryPath))
            throw new InvalidOperationException("El archivo descargado no contenía aria2c.");

        if (!isWin)
        {
            try
            {
                File.SetUnixFileMode(BinaryPath,
                    UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute |
                    UnixFileMode.GroupRead | UnixFileMode.GroupExecute |
                    UnixFileMode.OtherRead | UnixFileMode.OtherExecute);
            }
            catch { /* noop */ }
        }
    }

    private static void ExtractEngine(string archive, bool isWin)
    {
        var workDir = Path.Combine(AppPaths.TempDir, "extract_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(workDir);
        try
        {
            if (isWin)
                ZipFile.ExtractToDirectory(archive, workDir);
            else
                ExtractTarGz(archive, workDir);
            var exe = Directory.GetFiles(workDir, isWin ? "aria2c.exe" : "aria2c", SearchOption.AllDirectories)
                .FirstOrDefault();
            if (exe == null) throw new InvalidOperationException("aria2c no encontrado en el paquete.");

            File.Copy(exe, AppPaths.EngineBinary, overwrite: true);
        }
        finally
        {
            try { Directory.Delete(workDir, recursive: true); } catch { /* noop */ }
        }
    }

    private static void ExtractTarGz(string archive, string dest)
    {
        // GNU tar está presente en cualquier Linux; extrae el paquete en el directorio de trabajo
        var psi = new ProcessStartInfo("tar")
        {
            UseShellExecute = false,
            CreateNoWindow = true,
        };
        psi.ArgumentList.Add("-xf");
        psi.ArgumentList.Add(archive);
        psi.ArgumentList.Add("-C");
        psi.ArgumentList.Add(dest);
        using var p = Process.Start(psi)
            ?? throw new InvalidOperationException("No se pudo ejecutar tar.");
        if (!p.WaitForExit(60_000))
        {
            try { p.Kill(); } catch { /* noop */ }
            throw new InvalidOperationException("tar agotó el tiempo de extracción.");
        }
        if (p.ExitCode != 0)
            throw new InvalidOperationException($"tar devolvió código {p.ExitCode}.");
    }

    /// <summary>Arranca el demonio aria2c (si falta, deja estado Missing para que la UI muestre el botón de descarga).</summary>
    public async Task<bool> EnsureStartedAsync(Settings settings)
    {
        if (IsRunning) return true;
        if (!BinaryPresent)
        {
            State = EngineState.Missing;
            return false;
        }

        State = EngineState.Starting;
        LastError = null;
        lock (_stderr) _stderr.Clear();

        try
        {
            // Sanea sesiones huérfanas: si hay demonio vivo por un cierre brusco, mátarlo.
            await KillOrphanAsync();

            foreach (var port in new ushort[] { 6800, 6801, 6802, 6803, 6804 })
            {
                var psi = new ProcessStartInfo(BinaryPath)
                {
                    UseShellExecute = false,
                    CreateNoWindow = true,
                    RedirectStandardOutput = false,
                    RedirectStandardError = true,
                };
                foreach (var a in BuildArgs(settings, port)) psi.ArgumentList.Add(a);

                var proc = Process.Start(psi);
                if (proc == null) continue;
                _proc = proc;
                AssignToJob(proc);

                var rpc = new AriaRpc(port, Secret);

                // Captura stderr para diagnóstico (aria2c escribe ahí sus errores de arranque)
                _ = Task.Run(async () =>
                {
                    try
                    {
                        string? line;
                        while ((line = await proc.StandardError.ReadLineAsync()) != null)
                            lock (_stderr) _stderr.Append(line).Append(" · ");
                    }
                    catch { /* el proceso murió */ }
                });

                // Esperar a que la RPC responda (~2.5 s por puerto)
                for (int i = 0; i < 25; i++)
                {
                    await Task.Delay(100);
                    var v = await rpc.TryGetVersionAsync();
                    if (v != null)
                    {
                        try { File.WriteAllText(AppPaths.PidFile, proc.Id.ToString()); } catch { /* noop */ }
                        _rpc = rpc;
                        Port = port;
                        Version = v;
                        State = EngineState.Running;
                        return true;
                    }
                    if (proc.HasExited) break;
                }

                // no arrancó en este puerto: limpiar y probar el siguiente
                try { if (!proc.HasExited) proc.Kill(entireProcessTree: true); } catch { /* noop */ }
                _proc = null;
            }

            State = EngineState.Error;
            LastError = LastStderr();
            _rpc = null;
            return false;
        }
        catch (Exception ex)
        {
            State = EngineState.Error;
            LastError = ex.Message;
            return false;
        }
    }

    public ushort Port { get; private set; }

    private List<string> BuildArgs(Settings s, ushort port) => new()
    {
        "--enable-rpc",
        "--rpc-listen-all=false",
        $"--rpc-listen-port={port}",
        $"--rpc-secret={Secret}",
        $"--max-concurrent-downloads={Math.Clamp(s.MaxConcurrentDownloads, 1, 20)}",
        // Máxima paralelización: hasta 16 conexiones por servidor y 16 segmentos por descarga
        "--max-connection-per-server=16",
        "--split=16",
        "--min-split-size=1M",
        "--continue=true",
        "--auto-file-renaming=false",
        "--allow-overwrite=true",
        // Windows: "none" — falloc/prealloc usan SetFileValidData (requiere privilegios de
        // administrador; sin él, aria2 rellena de ceros el archivo completo y BLOQUEA el
        // demonio entero: todo queda "en cola" sin avance). Linux: prealloc va perfecto.
        (OperatingSystem.IsWindows() ? "--file-allocation=none" : "--file-allocation=prealloc"),
        "--disk-cache=32M",
        "--timeout=60",
        "--connect-timeout=30",
        "--max-tries=5",
        "--retry-wait=3",
        "--console-log-level=warn",
        "--log-level=warn",
        $"--log={AppPaths.EngineLog}",
        // La sesión de aria2 es un respaldo; el registro propio de la app es la fuente de verdad
        // para reanudar tras cerrar (por eso no se usa --input-file: evitar dobles descargas).
        $"--save-session={AppPaths.SessionFile}",
        "--follow-torrent=true",
    };

    private static async Task KillOrphanAsync()
    {
        try
        {
            if (!File.Exists(AppPaths.PidFile)) return;
            var pid = int.Parse(File.ReadAllText(AppPaths.PidFile));
            try
            {
                var p = Process.GetProcessById(pid);
                if (Path.GetFileName(p.ProcessName).StartsWith("aria2c", StringComparison.OrdinalIgnoreCase))
                {
                    p.Kill(entireProcessTree: true);
                    await p.WaitForExitAsync().ContinueWith(_ => { });
                }
            }
            catch { /* no existe */ }
            File.Delete(AppPaths.PidFile);
        }
        catch { /* noop */ }
    }

    /// <summary>Parada ordenada: aria2.shutdown guarda la sesión para reanudar tras cerrar la app.</summary>
    public async Task StopDaemonForShutdownAsync()
    {
        try
        {
            if (_rpc != null)
            {
                try { await _rpc.CallAsync("aria2.shutdown", new object?[0], new CancellationTokenSource(2000).Token); }
                catch { /* ya estará muerta */ }
            }
            if (_proc is { HasExited: false })
            {
                using var cts = new CancellationTokenSource(4000);
                try { await _proc.WaitForExitAsync(cts.Token); }
                catch { _proc.Kill(entireProcessTree: true); }
            }
        }
        catch { /* noop */ }
        finally
        {
            _rpc = null;
            State = EngineState.Missing;
            try { if (File.Exists(AppPaths.PidFile)) File.Delete(AppPaths.PidFile); } catch { /* noop */ }
        }
    }

    public void Dispose()
    {
        try { if (_proc is { HasExited: false }) _proc.Kill(entireProcessTree: true); } catch { /* noop */ }
        _rpc = null;
    }

    // ------------------------------------------------------------------ garantía de cierre
    //
    // Job object de Windows con KILL_ON_JOB_CLOSE: el propio SO mata a aria2c si el
    // proceso de la app muere de cualquier forma (Task Manager, apagado, crash).
    // La parada ordenada (aria2.shutdown) sigue siendo el camino normal; esto es la red.

    private IntPtr _job = IntPtr.Zero;

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_BASIC_LIMIT_INFORMATION
    {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize;
        public UIntPtr MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public uint Affinity;
        public uint PriorityClass;
        public uint SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct IO_COUNTERS
    {
        public ulong ReadOperationCount, WriteOperationCount, OtherOperationCount;
        public ulong ReadTransferCount, WriteTransferCount, OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION
    {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
        public IO_COUNTERS IoInfo;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryUsed;
        public UIntPtr PeakJobMemoryUsed;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr CreateJobObjectW(IntPtr lpJobAttributes, string? lpName);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetInformationJobObject(IntPtr job, int infoClass, IntPtr lpJobObjectInfo, uint cbJobObjectInfoLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool IsProcessInJob(IntPtr process, IntPtr job, out bool result);

    private const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x2000;

    private void AssignToJob(Process proc)
    {
        if (!OperatingSystem.IsWindows()) return;
        try
        {
            if (_job == IntPtr.Zero)
            {
                _job = CreateJobObjectW(IntPtr.Zero, "Aria2DownloadsBooster_daemon_job");
                if (_job == IntPtr.Zero)
                {
                    UiLog.Write($"CreateJobObjectW falló err={Marshal.GetLastWin32Error()}");
                    return;
                }
                // El tamano real de JOBOBJECT_EXTENDED_LIMIT_INFORMATION varía entre headers del SDK:
                // probamos tamaños candidatos (LimitFlags siempre en offset 16; todo lo demás, 0).
                var ptr = Marshal.AllocHGlobal(256);
                try
                {
                    for (var k = 0; k < 256; k += 4) Marshal.WriteInt32(ptr + k, 0);
                    Marshal.WriteInt32(ptr + 16, (int)JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE);
                    int[] sizes = { 144, 136, Marshal.SizeOf<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() };
                    var okSize = 0;
                    foreach (var size in sizes)
                    {
                        if (SetInformationJobObject(_job, 9, ptr, (uint)size)) { okSize = size; break; }
                    }
                    UiLog.Write(okSize > 0
                        ? $"job kill-on-close activo (struct {okSize}B)"
                        : $"job kill-on-close NO aplicable err={Marshal.GetLastWin32Error()}");
                }
                finally { Marshal.FreeHGlobal(ptr); }
            }
            var assigned = AssignProcessToJobObject(_job, proc.Handle);
            if (!assigned)
                UiLog.Write($"AssignProcessToJobObject falló err={Marshal.GetLastWin32Error()}");
        }
        catch (Exception ex) { UiLog.Write("AssignToJob exc: " + ex.Message); }
    }
}
