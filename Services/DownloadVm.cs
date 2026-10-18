using System;
using System.Windows.Input;
using Avalonia.Media;
using CommunityToolkit.Mvvm.ComponentModel;

namespace AriaXtr.Services;

/// <summary>ViewModel de una descarga individual para las tarjetas de la lista.</summary>
public partial class DownloadVm : ObservableObject
{
    public DownloadEntry Entry { get; }

    /// <summary>Comandos enlazados a los botones de la tarjeta (los inyecta MainWindowViewModel).</summary>
    public ICommand? PauseCommand { get; set; }
    public ICommand? ResumeCommand { get; set; }
    public ICommand? OpenFolderCommand { get; set; }
    public ICommand? DeleteCommand { get; set; }

    [ObservableProperty] private string _statusKey = "stopped";
    [ObservableProperty] private string _statusText = "";
    [ObservableProperty] private string _actionText = "";
    [ObservableProperty] private double _progress;          // 0..1
    [ObservableProperty] private string _sizeText = "";
    [ObservableProperty] private string _speedText = "";
    [ObservableProperty] private string _etaText = "";
    [ObservableProperty] private string _threadsText = "";
    [ObservableProperty] private string _errorText = "";
    [ObservableProperty] private SolidColorBrush _badgeBrush = new(Color.FromRgb(0x6B, 0x72, 0x8A));
    [ObservableProperty] private bool _isComplete;
    [ObservableProperty] private bool _canPause;
    [ObservableProperty] private bool _canResume;
    [ObservableProperty] private bool _isDetailVisible;

    // Últimos datos crudos para derivar la acción en curso
    private long _lastTotal;
    private long _lastDone;
    private double _lastSpeed;
    private int _connsUsed;
    private bool _retryHint;
    private bool _slotsFree = true;

    public string Id => Entry.Id;
    public string Name => Entry.Name;

    // Detalles (fijos)
    public string UrlsText => string.Join("\n", Entry.Urls);
    public string DirText => Entry.Dir;
    public string AddedText => Entry.AddedAt.ToString("dd/MM/yyyy HH:mm:ss");

    [CommunityToolkit.Mvvm.Input.RelayCommand]
    private void ToggleDetail() => IsDetailVisible = !IsDetailVisible;

    public DownloadVm(DownloadEntry entry)
    {
        Entry = entry;
        ApplyStatus(entry.Status, error: entry.Error);
    }

    public void UpdateProgress(long completed, long total, double speedBps, int connections, bool slotsFree)
    {
        Entry.Total = total;
        _lastTotal = total;
        _lastDone = completed;
        _lastSpeed = speedBps;
        _connsUsed = connections;
        _slotsFree = slotsFree;

        if (total > 0)
        {
            Progress = Math.Clamp((double)completed / total, 0, 1);
            var eta = speedBps > 1 && completed < total
                ? TimeSpan.FromSeconds((total - completed) / speedBps)
                : TimeSpan.Zero;
            EtaText = eta > TimeSpan.Zero ? FormatEta(eta) : "—";
        }
        else
        {
            Progress = 1; // indeterminado: barra llena animable
            EtaText = "";
        }
        SizeText = Loc.FormatSize(completed) + (total > 0 ? " / " + Loc.FormatSize(total) : "");
        SpeedText = Loc.FormatSpeed(speedBps);
        ThreadsText = $"{connections}/{Entry.Connections} {Loc.ThreadsWord}";
        RecomputeAction();
    }

    public void ApplyStatus(string statusKey, string? error = null, bool retryHint = false)
    {
        Entry.Status = statusKey;
        Entry.Error = error;
        StatusKey = statusKey;
        _retryHint = retryHint;
        StatusText = statusKey == "waiting" && retryHint ? Loc.Retrying : Loc.StatusText(statusKey);
        ErrorText = statusKey == "error" && !string.IsNullOrWhiteSpace(error) ? error : "";

        (IsComplete, CanPause, CanResume, BadgeBrush) = statusKey switch
        {
            "active" => (false, true, false, Brush("#22C55E")),
            "waiting" => (false, true, false, Brush(_retryHint ? "#F59E0B" : "#A78BFA")),
            "paused" => (false, false, true, Brush("#F59E0B")),
            "stopped" => (false, false, true, Brush("#6B728A")),
            "error" => (false, false, true, Brush("#EF4444")),
            "complete" => (true, false, false, Brush("#38BDF8")),
            _ => (false, false, true, Brush("#6B728A")),
        };
        RecomputeAction();
    }

    /// <summary>Traduce el estado crudo de aria2 a "qué está haciendo ahora mismo".</summary>
    private void RecomputeAction()
    {
        var en = Loc.IsEnglish;
        ActionText = StatusKey switch
        {
            "active" => _lastTotal == 0
                ? (en ? "Downloading (size still unknown)…" : "Descargando (tamaño aún desconocido)…")
                : _lastSpeed > 0
                    ? (en ? "Receiving data" : "Recibiendo datos")
                    : (en ? "Holding connections, waiting for data…" : "Conexiones abiertas, esperando datos…"),
            "waiting" => _retryHint
                ? (_lastTotal == 0 && _lastDone == 0
                    ? (en ? "Retry: resolving host/URL…" : "Reintento: resolviendo host/URL…")
                    : Loc.Retrying)
                : (_slotsFree
                    ? (en ? "Preparing (DNS / redirects / space allocation)…" : "Preparando (DNS / redirecciones / reservando espacio)…")
                    : (en ? "Queued, waiting for a free slot" : "En cola, esperando hueco libre")),
            "paused" => en ? "Paused by user" : "Pausada por el usuario",
            "stopped" => en ? "Not running — press ▶ to resume" : "Detenida — pulsa ▶ para reanudar",
            "error" => en ? "Stopped by error — press ▶ to retry" : "Detenida por error — pulsa ▶ para reintentar",
            "complete" => en ? "Saved" : "Guardada",
            _ => "",
        };
    }

    private static SolidColorBrush Brush(string hex) =>
        new(Color.Parse(hex));

    private static string FormatEta(TimeSpan t)
    {
        if (t.TotalHours >= 1) return $"{(int)t.TotalHours}h {t:mm}m";
        if (t.TotalMinutes >= 1) return $"{(int)t.TotalMinutes}m {t:ss}s";
        return $"{t.Seconds}s";
    }
}
