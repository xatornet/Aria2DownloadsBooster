using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using AriaXtr.Services;
using AriaXtr.Views;

namespace AriaXtr.ViewModels;

public partial class MainWindowViewModel : ObservableObject
{
    private readonly RegistryService _reg = new();
    private readonly EngineService _engine = new();
    private readonly DispatcherTimer _timer;
    private bool _refreshing;
    private DateTime _nextStartAttempt = DateTime.MinValue;
    private bool _dirty;

    public ObservableCollection<DownloadVm> Active { get; } = new();
    public ObservableCollection<DownloadVm> Completed { get; } = new();

    [ObservableProperty] private string _engineStatusText = "";
    [ObservableProperty] private bool _engineBannerVisible;
    [ObservableProperty] private string _engineBannerText = "";
    [ObservableProperty] private bool _installButtonVisible = true;
    [ObservableProperty] private bool _engineInstalling;
    [ObservableProperty] private double _installProgress;
    [ObservableProperty] private bool _installIndeterminate;
    [ObservableProperty] private string _globalSpeedText = "";
    [ObservableProperty] private string _pollHeartbeatText = "";
    [ObservableProperty] private string _lastActionMessage = "";
    [ObservableProperty] private bool _hasActive;
    [ObservableProperty] private bool _hasCompleted;
    [ObservableProperty] private bool _showCompletedList = true;

    public string FooterText { get; } = $"Aria2 Downloads Booster v1.1 · datos: {AppPaths.BaseDir}";

    public string DefaultDir => _reg.Settings.DefaultDir
        ?? Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
    public int DefaultConnections => Settings.ClampConnections(_reg.Settings.DefaultConnections);

    public Settings AppSettings => _reg.Settings;

    /// <summary>Devuelve true solo la PRIMERA vez que se oculta a la bandeja (persistido).</summary>
    public bool ShouldShowTrayHint()
    {
        if (_reg.Settings.TrayHintShown) return false;
        _reg.Settings.TrayHintShown = true;
        _reg.SaveSettings();
        return true;
    }

    public MainWindowViewModel()
    {
        foreach (var e in _reg.Entries.OrderBy(x => x.AddedAt))
        {
            var vm = new DownloadVm(e);
            AttachCommands(vm);
            (e.Completed ? Completed : Active).Add(vm);
        }
        // Tras un cierre sin parada limpia, nada vive en aria2: aparecen como detenidas.
        foreach (var vm in Active)
            if (vm.StatusKey is "active" or "waiting")
                vm.ApplyStatus("stopped");

        UpdateCollectionFlags();
        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _timer.Tick += (_, _) => _ = RefreshAsync();
        _ = InitializeAsync();
        _timer.Start();
    }

    private async Task InitializeAsync()
    {
        try { await _engine.EnsureStartedAsync(_reg.Settings); } catch { /* banner lo dirá */ }
        await RefreshAsync();
    }

    // ----------------------------------------------------------------- polling

    private async Task RefreshAsync()
    {
        if (_refreshing) return;
        _refreshing = true;
        try
        {
            if (!_engine.IsRunning)
            {
                if (_engine.State is EngineState.Missing or EngineState.Error
                    && DateTime.Now < _nextStartAttempt)
                {
                    UpdateBanner();
                    return;
                }
                var ok = false;
                try { ok = await _engine.EnsureStartedAsync(_reg.Settings); } catch { /* noop */ }
                if (_engine.State is EngineState.Error)
                    _nextStartAttempt = DateTime.Now.AddSeconds(30);
                if (!ok || !_engine.IsRunning)
                {
                    foreach (var vm in Active.Where(v => v.StatusKey is "active" or "waiting"))
                        vm.ApplyStatus("stopped");
                    GlobalSpeedText = "";
                    UpdateBanner();
                    SaveIfDirty();
                    return;
                }
            }

            await QueryStatusesAsync();
            UpdateBanner();
            SaveIfDirty();
        }
        finally { _refreshing = false; }
    }

    private static readonly string[] StatusFields =
        { "status", "totalLength", "completedLength", "downloadSpeed", "connections", "errorMessage" };

    private async Task QueryStatusesAsync()
    {
        var rpc = _engine.Rpc;
        if (rpc == null) return;

        var targets = Active.Select(v => v.Entry)
            .Where(e => e.Gid != null)
            .ToList();

        // Paso 1: consultar al demonio (tellStatus por descarga + tellActive)
        var seen = new List<(DownloadEntry entry, string status, long total, long done,
                             double speed, int conns, string? err)>();
        int daemonActive = -1;

        foreach (var chunk in targets.Chunk(100))
        {
            var calls = new List<(string Method, object?[] Args)>(
                chunk.Select(e => ("aria2.tellStatus", new object?[] { e.Gid, StatusFields })));
            calls.Add(("aria2.tellActive", Array.Empty<object?>()));
            // OJO: aria2.tellWaiting está roto dentro de system.multicall con --rpc-secret
            // (siempre "parameter at 1 required", verificado contra aria2 1.37.0). El conteo de
            // "cola" se obtiene de los propios tellStatus de seen (registry = fuente de verdad).

            JsonElement res;
            try
            {
                res = await rpc.MulticallAsync(calls);
            }
            catch (AriaRpcException ex)
            {
                LogPollError(ex.Message);
                return;
            }

            var elems = res.EnumerateArray().ToArray();
            if (elems.Length >= 1)
                daemonActive = CountOkArray(elems[^1]); // último elemento = tellActive

            foreach (var (entry, el) in chunk.Zip(elems))
            {
                if (MulticallResult.TryGet(el, out var st))
                {
                    seen.Add((entry,
                        st.TryGetProperty("status", out var s) ? s.GetString() ?? "stopped" : "stopped",
                        ParseLen(st, "totalLength"),
                        ParseLen(st, "completedLength"),
                        ParseDouble(st, "downloadSpeed"),
                        (int)ParseDouble(st, "connections"),
                        st.TryGetProperty("errorMessage", out var em) ? em.GetString() : null));
                }
                else
                {
                    // sub-llamada con error: gid desconocido (demonio reiniciado) u otro fallo
                    var msg = MulticallResult.GetError(el);
                    if (msg != null) UiLog.Write($"tellStatus gid={entry.Gid} error: {msg}");
                    seen.Add((entry, "__unknown__", 0, 0, 0, 0, null));
                }
            }
        }

        // Paso 2: aplicar a las VMs. "waiting" con hueco libre ⇒ realmente está reintentando
        var activeNow = daemonActive >= 0 ? daemonActive : seen.Count(x => x.status == "active");
        var slotsFree = activeNow < Math.Max(1, _reg.Settings.MaxConcurrentDownloads);
        double totalSpeed = 0;

        foreach (var (entry, status, total, done, speed, conns, err) in seen)
        {
            var vm = FindVm(entry.Id);
            if (vm == null) continue;

            switch (status)
            {
                case "__unknown__":
                    // El demonio perdió este gid (craseó o se reinició). Auto-sanear siempre,
                    // también si estaba "paused" (si no, el gid muerto queda clavado y spamea).
                    if (entry.Gid == null) break;
                    var wasDownloading = vm.StatusKey is "active" or "waiting";
                    entry.Gid = null;
                    _dirty = true;
                    if (wasDownloading)
                    {
                        // Estaba descargando: re-añadir automáticamente (continúa vía .aria2)
                        UiLog.Write($"Gid perdido ({vm.Name}): re-añadiendo automáticamente");
                        vm.ApplyStatus("waiting");
                        _ = ReaddLostAsync(vm);
                    }
                    else
                    {
                        vm.ApplyStatus("stopped");
                    }
                    break;
                case "complete":
                    entry.Completed = true;
                    vm.UpdateProgress(total, total, 0, 0, slotsFree);
                    vm.ApplyStatus("complete");
                    Active.Remove(vm);
                    Completed.Add(vm);
                    _dirty = true;
                    _ = TryRemoveResultAsync(rpc, entry.Gid);
                    // aviso solo si la ventana está oculta (minimizada a la bandeja)
                    var uiHidden = Application.Current?.ApplicationLifetime
                        is IClassicDesktopStyleApplicationLifetime dl && dl.MainWindow?.IsVisible == false;
                    if (uiHidden)
                    {
                        var doneName = entry.Name;
                        var doneSize = Loc.FormatSize(total);
                        Dispatcher.UIThread.Post(() =>
                            Toast.Show(Loc.DownloadDoneTitle, $"{doneName} · {doneSize}", Tray.OpenMainWindow));
                    }
                    break;
                case "error":
                    vm.UpdateProgress(done, total, 0, conns, slotsFree);
                    vm.ApplyStatus("error", err);
                    _dirty = true;
                    break;
                default: // active | waiting | paused
                    vm.UpdateProgress(done, total, speed, conns, slotsFree);
                    vm.ApplyStatus(status, retryHint: status == "waiting" && slotsFree);
                    if (status == "active") totalSpeed += speed;
                    break;
            }
        }

        // Latido: demuestra que el sondeo está vivo con los contadores del propio demonio
        var waitingNow = seen.Count(x => x.status == "waiting");
        var daemonInfo = daemonActive >= 0
            ? $"aria2: {daemonActive} act / {waitingNow} cola"
            : "aria2: sin datos";

        GlobalSpeedText = Active.Any(v => v.StatusKey == "active")
            ? $"{Loc.GlobalSpeed}: {Loc.FormatSize(totalSpeed)}/s"
            : "";
        PollHeartbeatText = $"{DateTime.Now:HH:mm:ss} · {daemonInfo}";
        _lastPollError = null; // sondeo OK: resetea el filtro de log de errores
        UpdateCollectionFlags();
    }

    private string? _lastPollError;

    private void LogPollError(string msg)
    {
        if (_lastPollError == msg) return; // no spamear el log cada segundo con lo mismo
        _lastPollError = msg;
        UiLog.Write("POLL ERROR: " + msg);
    }

    /// <summary>Re-añade una descarga cuyo gid desapareció (demonio muerto/reiniciado).
    /// aria2 continúa con el archivo de control .aria2 que quedó en disco.</summary>
    private async Task ReaddLostAsync(DownloadVm vm)
    {
        try
        {
            var rpc = _engine.Rpc;
            if (rpc == null || vm.Entry.Gid != null) return; // ya fue re-añadida
            await ReaddAsync(vm, rpc);
        }
        catch (Exception ex)
        {
            UiLog.Write($"Re-add automático FALLÓ ({vm.Name}): {ex.GetType().Name}: {ex.Message}");
            vm.ApplyStatus("error", ex.Message);
            _dirty = true;
        }
    }

    private static int CountOkArray(JsonElement el) =>
        MulticallResult.TryGet(el, out var v) && v.ValueKind == JsonValueKind.Array
            ? v.GetArrayLength() : -1;

    private static long ParseLen(JsonElement st, string prop) =>
        st.TryGetProperty(prop, out var v) && v.ValueKind == JsonValueKind.String
        && long.TryParse(v.GetString(), out var n) ? n
        : st.TryGetProperty(prop, out var v2) && v2.TryGetInt64(out var n2) ? n2 : 0;

    private static double ParseDouble(JsonElement st, string prop) =>
        st.TryGetProperty(prop, out var v) && v.ValueKind == JsonValueKind.String
        && double.TryParse(v.GetString(), System.Globalization.CultureInfo.InvariantCulture, out var n) ? n
        : st.TryGetProperty(prop, out var v2) && v2.TryGetDouble(out var n2) ? n2 : 0;

    private static async Task TryRemoveResultAsync(AriaRpc rpc, string? gid)
    {
        if (gid == null) return;
        try { await rpc.CallAsync("aria2.removeDownloadResult", gid); } catch { /* noop */ }
    }
}
