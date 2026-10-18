using System;
using System.IO;

namespace AriaXtr.Services;

/// <summary>Rutas de datos de la app (motor aria2, registro, ajustes).</summary>
public static class AppPaths
{
    /// <summary>Directorio de datos; puede redirigirse con la variable ARIA_XTR_DATA
    /// (útil para pruebas/portable). De lo contrario, %APPDATA% (Win) o ~/.local/share no: AppData uniforme.</summary>
    public static string BaseDir { get; } =
        Environment.GetEnvironmentVariable("ARIA_XTR_DATA") is { Length: > 0 } redirect
            ? redirect
            : Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                "Aria-XTRDownloader");

    public static string EngineDir => Path.Combine(BaseDir, "engine");

    public static string RegistryFile => Path.Combine(BaseDir, "registry.json");
    public static string SettingsFile => Path.Combine(BaseDir, "settings.json");
    public static string UiLogFile => Path.Combine(BaseDir, "ui.log");
    public static string SessionFile => Path.Combine(BaseDir, "aria2-session.txt");
    public static string EngineLog => Path.Combine(BaseDir, "aria2c.log");
    public static string PidFile => Path.Combine(BaseDir, "aria2c.pid");
    public static string TempDir => Path.Combine(BaseDir, "temp");

    public static string EngineBinary
    {
        get
        {
            var name = OperatingSystem.IsWindows() ? "aria2c.exe" : "aria2c";
            return Path.Combine(EngineDir, name);
        }
    }

    public static void EnsureBase()
    {
        Directory.CreateDirectory(BaseDir);
        Directory.CreateDirectory(EngineDir);
        Directory.CreateDirectory(TempDir);
    }
}
