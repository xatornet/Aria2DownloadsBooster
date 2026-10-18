using System;
using System.Collections.Generic;

namespace AriaXtr.Services;

public class Settings
{
    public int MaxConcurrentDownloads { get; set; } = 3;
    public int DefaultConnections { get; set; } = 16;
    public string? DefaultDir { get; set; }
    /// <summary>Al cerrar la ventana, minimizar a la bandeja y seguir descargando.</summary>
    public bool MinimizeToTray { get; set; } = true;
    /// <summary>Ya se mostró una vez el aviso de "sigue activo en segundo plano".</summary>
    public bool TrayHintShown { get; set; } = false;

    public static int ClampConnections(int n) => Math.Clamp(n, 1, 16);
}

public class DownloadEntry
{
    public string Id { get; set; } = Guid.NewGuid().ToString("N");
    public string? Gid { get; set; }
    public List<string> Urls { get; set; } = new();
    public string Dir { get; set; } = "";
    public string? Out { get; set; }
    public string Name { get; set; } = "";
    /// <summary>active | waiting | paused | stopped | error | complete</summary>
    public string Status { get; set; } = "stopped";
    public long Total { get; set; }
    public bool Completed { get; set; }
    public string? Error { get; set; }
    public int Connections { get; set; } = 16;
    public DateTime AddedAt { get; set; } = DateTime.Now;
}
