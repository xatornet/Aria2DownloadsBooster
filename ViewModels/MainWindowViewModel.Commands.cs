using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using CommunityToolkit.Mvvm.Input;
using AriaXtr.Services;
using AriaXtr.Views;

namespace AriaXtr.ViewModels;

public partial class MainWindowViewModel
{
    // ----------------------------------------------------------------- banner motor

    private void UpdateBanner()
    {        switch (_engine.State)
        {
            case EngineState.Running:
                EngineBannerVisible = false;
                EngineStatusText = $"{Loc.EngineReady} v{_engine.Version}";
                break;
            case EngineState.Missing:
                EngineBannerVisible = true;
                EngineBannerText = Loc.EngineMissingText;
                InstallButtonVisible = true;
                EngineStatusText = Loc.EngineMissingText;
                break;
            case EngineState.Starting:
                EngineBannerVisible = true;
                EngineBannerText = Loc.EngineStarting;
                InstallButtonVisible = false;
                EngineStatusText = Loc.EngineStarting;
                break;
            case EngineState.Error:
                EngineBannerVisible = true;
                var cause = _engine.LastError;
                EngineBannerText = Loc.EngineError
                    + (string.IsNullOrWhiteSpace(cause) ? " — " + AppPaths.EngineLog : ": " + cause);
                InstallButtonVisible = true;
                EngineStatusText = Loc.EngineError;
                break;
        }
    }

    private void UpdateCollectionFlags()
    {
        HasActive = Active.Count > 0;
        HasCompleted = Completed.Count > 0;
    }

    private DownloadVm? FindVm(string id) =>
        Active.FirstOrDefault(v => v.Id == id) ?? Completed.FirstOrDefault(v => v.Id == id);

    private static Window? GetMainWindow() =>
        (Application.Current?.ApplicationLifetime as IClassicDesktopStyleApplicationLifetime)?.MainWindow;

    private void AttachCommands(DownloadVm vm)
    {
        vm.PauseCommand = PauseCommand;
        vm.ResumeCommand = ResumeCommand;
        vm.OpenFolderCommand = OpenFolderCommand;
        vm.DeleteCommand = DeleteCommand;
    }

    private void SaveIfDirty()
    {
        if (!_dirty) return;
        _dirty = false;
        _reg.Save();
    }

    private CancellationTokenSource? _actionMsgCts;

    /// <summary>Mensaje visible ~9 s en la cabecera (éxito/error de una acción concreta).</summary>
    public void FlashActionMessage(string msg)
    {
        UiLog.Write("UI: " + msg);
        _actionMsgCts?.Cancel();
        var cts = _actionMsgCts = new CancellationTokenSource();
        LastActionMessage = msg;
        _ = Task.Delay(9000, cts.Token).ContinueWith(t =>
        {
            if (t is { IsCanceled: false })
                LastActionMessage = "";
        });
    }

    // ----------------------------------------------------------------- motor: instalar aria2

    [RelayCommand]
    private async Task InstallEngineAsync()
    {
        if (EngineInstalling) return;
        EngineInstalling = true;
        InstallProgress = 0;
        InstallIndeterminate = true;
        EngineBannerText = Loc.InstallingEngine;

        var progress = new Progress<(long received, long total)>(p =>
        {
            InstallIndeterminate = p.total <= 0;
            if (p.total > 0) InstallProgress = p.received * 100.0 / p.total;
            EngineBannerText = $"{Loc.InstallingEngine}  {Loc.FormatSize(p.received)}"
                               + (p.total > 0 ? " / " + Loc.FormatSize(p.total) : "");
        });

        try
        {
            await _engine.DownloadEngineAsync(progress);
            EngineBannerText = Loc.EngineStarting;
            _nextStartAttempt = DateTime.MinValue;
            await _engine.EnsureStartedAsync(_reg.Settings);
            await RefreshAsync();
        }
        catch (Exception ex)
        {
            EngineBannerText = Loc.EngineError + ": " + ex.Message;
            EngineStatusText = Loc.EngineError;
        }
        finally
        {
            EngineInstalling = false;
        }
    }

    // ----------------------------------------------------------------- alta de descargas

    [RelayCommand]
    private async Task AddDownloadAsync()
    {
        UiLog.Write("AddDownload: abriendo diálogo");
        var win = new AddDownloadWindow(DefaultDir, DefaultConnections);
        var ok = await win.ShowDialog<bool>(GetMainWindow()!);
        UiLog.Write($"AddDownload: diálogo cerrado (aceptado={ok})");
        if (!ok) return;

        try
        {
            await AddUriAsync(win.ResultUrls, win.ResultDir, win.ResultFileName, win.ResultConnections);
        }
        catch (Exception ex)
        {
            UiLog.Write($"AddDownload ERROR: {ex.GetType().Name}: {ex.Message}");
            FlashActionMessage("⚠ No se pudo añadir: " + ex.Message);
        }
    }

    private async Task AddUriAsync(List<string> urls, string dir, string? filename, int connections)
    {
        // duplicado: misma URL principal ya en la lista (sin completar)
        var dup = _reg.Entries.FirstOrDefault(e =>
            !e.Completed && e.Urls.Count > 0 &&
            string.Equals(e.Urls[0], urls[0], StringComparison.OrdinalIgnoreCase));
        if (dup != null)
        {
            FlashActionMessage("ℹ Esa URL ya está en la lista (" + dup.Name + ")");
            return;
        }

        await _engine.EnsureStartedAsync(_reg.Settings);
        var rpc = _engine.Rpc ?? throw new InvalidOperationException(Loc.EngineError);

        connections = Settings.ClampConnections(connections);
        var outName = string.IsNullOrWhiteSpace(filename) ? DeriveNameFromUrl(urls[0]) : filename!.Trim();

        var opts = new Dictionary<string, string>
        {
            ["dir"] = dir,
            ["out"] = outName,
            ["continue"] = "true",
            ["maxConnectionPerServer"] = connections.ToString(),
            ["split"] = connections.ToString(),
            ["autoFileRenaming"] = "false",
            ["allowOverwrite"] = "true",
        };

        UiLog.Write($"addUri → {urls[0]} dir={dir} out={outName} x{connections}");
        var gid = (await rpc.CallAsync("aria2.addUri", new object?[] { urls, opts })).GetString()
                  ?? throw new InvalidOperationException("aria2 no devolvió GID.");
        UiLog.Write($"addUri OK gid={gid}");

        var entry = new DownloadEntry
        {
            Gid = gid,
            Urls = urls,
            Dir = dir,
            Out = outName,
            Name = outName,
            Connections = connections,
            Status = "waiting",
        };
        _reg.Entries.Add(entry);
        _reg.Save();

        var vm = new DownloadVm(entry);
        AttachCommands(vm);
        vm.ApplyStatus("waiting");
        Active.Add(vm);
        UpdateCollectionFlags();
        FlashActionMessage("✔ Añadida: " + outName);
    }

    public static string DeriveNameFromUrl(string url)
    {
        try
        {
            var uri = new Uri(url);
            var seg = uri.Segments.LastOrDefault(s => s.Length > 1)?.TrimEnd('/');
            var name = string.IsNullOrEmpty(seg) ? uri.Host : Uri.UnescapeDataString(seg);
            foreach (var c in Path.GetInvalidFileNameChars())
                name = name.Replace(c, '_');
            return name;
        }
        catch
        {
            return "descarga-" + DateTime.Now.ToString("yyyyMMdd-HHmmss");
        }
    }

    // ----------------------------------------------------------------- pausar / reanudar

    [RelayCommand]
    private async Task PauseAsync(DownloadVm? vm)
    {
        if (vm?.Entry.Gid == null) return;
        UiLog.Write($"Pause gid={vm.Entry.Gid} name={vm.Name}");
        try
        {
            var rpc = _engine.Rpc ?? throw new InvalidOperationException(Loc.EngineError);
            try
            {
                await rpc.CallAsync("aria2.forcePause", vm.Entry.Gid);
            }
            catch (AriaRpcException ex) when (
                ex.Message.Contains("paused now", StringComparison.OrdinalIgnoreCase))
            {
                // fase transitoria (arranque/handshake): aria2 aún no puede pausar. Reintenta.
                await Task.Delay(1200);
                await rpc.CallAsync("aria2.forcePause", vm.Entry.Gid);
            }
            vm.ApplyStatus("paused");
            _dirty = true;
        }
        catch (AriaRpcException ex) when (ex.IsUnknownGid)
        {
            vm.Entry.Gid = null;
            vm.ApplyStatus("stopped");
            _dirty = true;
        }
        catch (AriaRpcException ex)
        {
            UiLog.Write($"Pause FALLÓ ({vm.Name}): {ex.Message}");
            FlashActionMessage("⚠ No se pudo pausar: " + ex.Message);
        }
    }

    [RelayCommand]
    private async Task ResumeAsync(DownloadVm? vm)
    {
        if (vm == null) return;
        UiLog.Write($"Resume gid={vm.Entry.Gid ?? "(ninguno)"} name={vm.Name}");
        try
        {
            await _engine.EnsureStartedAsync(_reg.Settings);
            var rpc = _engine.Rpc ?? throw new InvalidOperationException(Loc.EngineError);

            if (vm.Entry.Gid != null)
            {
                try
                {
                    await rpc.CallAsync("aria2.unpause", vm.Entry.Gid);
                    vm.ApplyStatus("waiting");
                    _dirty = true;
                    return;
                }
                catch (AriaRpcException ex) when (ex.IsUnknownGid)
                {
                    // El demonio se reinició: se re-añade; aria2 continúa con su archivo de control .aria2
                }
            }
            await ReaddAsync(vm, rpc);
        }
        catch (Exception ex)
        {
            vm.ApplyStatus("error", ex.Message);
            _dirty = true;
        }
    }

    private async Task ReaddAsync(DownloadVm vm, AriaRpc rpc)
    {
        var e = vm.Entry;
        var conn = Settings.ClampConnections(e.Connections);
        var opts = new Dictionary<string, string>
        {
            ["dir"] = e.Dir,
            ["continue"] = "true",
            ["maxConnectionPerServer"] = conn.ToString(),
            ["split"] = conn.ToString(),
            ["autoFileRenaming"] = "false",
            ["allowOverwrite"] = "true",
        };
        if (!string.IsNullOrWhiteSpace(e.Out)) opts["out"] = e.Out!;

        var gid = (await rpc.CallAsync("aria2.addUri", new object?[] { e.Urls, opts })).GetString();
        UiLog.Write($"Re-add OK ({vm.Name}) gid={gid} (continue=true, dir={e.Dir})");
        e.Gid = gid;
        vm.ApplyStatus("waiting");
        _dirty = true;
    }

    // ----------------------------------------------------------------- abrir / borrar

    [RelayCommand]
    private void OpenFolder(DownloadVm? vm)
    {
        if (vm == null) return;
        var dir = vm.Entry.Dir;
        if (!Directory.Exists(dir)) return;
        try
        {
            if (OperatingSystem.IsWindows())
                Process.Start(new ProcessStartInfo("explorer.exe") { ArgumentList = { dir } });
            else
                Process.Start(new ProcessStartInfo("xdg-open") { ArgumentList = { dir } });
        }
        catch { /* noop */ }
    }

    [RelayCommand]
    private async Task DeleteAsync(DownloadVm? vm)
    {
        if (vm == null) return;
        var dlg = new DeleteWindow(vm.Name);
        var ok = await dlg.ShowDialog<bool>(GetMainWindow()!);
        if (!ok) return;

        var gid = vm.Entry.Gid;
        if (_engine.IsRunning && gid != null)
        {
            try { await _engine.Rpc!.CallAsync("aria2.remove", gid); } catch { /* puede haber terminado */ }
            try { await _engine.Rpc!.CallAsync("aria2.removeDownloadResult", gid); } catch { /* noop */ }
        }

        if (dlg.DeleteFileRequested)
        {
            TryDeleteFile(Path.Combine(vm.Entry.Dir, vm.Entry.Name));
            TryDeleteFile(Path.Combine(vm.Entry.Dir, vm.Entry.Name + ".aria2"));
        }

        _reg.Entries.Remove(vm.Entry);
        Active.Remove(vm);
        Completed.Remove(vm);
        _reg.Save();
        UpdateCollectionFlags();
    }

    private static void TryDeleteFile(string path)
    {
        try { if (File.Exists(path)) File.Delete(path); } catch { /* en uso */ }
    }

    [RelayCommand]
    private void CleanupCompleted()
    {
        foreach (var vm in Completed.ToList())
            _reg.Entries.Remove(vm.Entry);
        Completed.Clear();
        _reg.Save();
        UpdateCollectionFlags();
    }

    // ----------------------------------------------------------------- ajustes

    [RelayCommand]
    private async Task OpenSettingsAsync()
    {
        var win = new SettingsWindow(_reg.Settings);
        var ok = await win.ShowDialog<bool>(GetMainWindow()!);
        if (!ok) return;

        _reg.Settings.MaxConcurrentDownloads = win.ResultMaxConcurrent;
        _reg.Settings.DefaultConnections = Settings.ClampConnections(win.ResultConnections);
        _reg.Settings.DefaultDir = win.ResultDefaultDir;
        _reg.Settings.MinimizeToTray = win.ResultMinimizeToTray;
        _reg.SaveSettings();
        Tray.SetEnabled(win.ResultMinimizeToTray);

        if (_engine.IsRunning)
        {
            try
            {
                await _engine.Rpc!.CallAsync("aria2.changeGlobalOption",
                    new Dictionary<string, string>
                    {
                        ["maxConcurrentDownloads"] = _reg.Settings.MaxConcurrentDownloads.ToString(),
                    });
            }
            catch { /* noop */ }
        }
    }

    // ----------------------------------------------------------------- apagado

    public void ShutdownEngine()
    {
        try
        {
            _timer.Stop();
            _engine.StopDaemonForShutdownAsync().Wait(TimeSpan.FromSeconds(4));
            _engine.Dispose();
        }
        catch { /* noop */ }
    }
}
