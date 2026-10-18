using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace AriaXtr.Services;

/// <summary>
/// Persistencia en JSON (AppData/Aria-XTRDownloader). Es la fuente de verdad para
/// mostrar y reanudar descargas cortadas aunque se cierre la app o mate aria2c.
/// </summary>
public sealed class RegistryService
{
    private static readonly JsonSerializerOptions Opts = new()
    {
        WriteIndented = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
    };

    public List<DownloadEntry> Entries { get; } = new();
    public Settings Settings { get; private set; } = new();

    public RegistryService()
    {
        AppPaths.EnsureBase();
        Load();
    }

    private void Load()
    {
        try
        {
            if (File.Exists(AppPaths.RegistryFile))
            {
                var list = JsonSerializer.Deserialize<List<DownloadEntry>>(
                    File.ReadAllText(AppPaths.RegistryFile), Opts);
                if (list != null) Entries.AddRange(list);
            }
            if (File.Exists(AppPaths.SettingsFile))
            {
                Settings = JsonSerializer.Deserialize<Settings>(
                    File.ReadAllText(AppPaths.SettingsFile), Opts) ?? new Settings();
            }
            Settings.DefaultDir ??= Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        }
        catch
        {
            // registro corrupto: empezar limpio, aria2 siempre puede continuar por archivos de control
            Entries.Clear();
            Settings = new Settings
            {
                DefaultDir = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile)
            };
        }
    }

    public void Save()
    {
        try
        {
            AppPaths.EnsureBase();
            File.WriteAllText(AppPaths.RegistryFile, JsonSerializer.Serialize(Entries, Opts));
        }
        catch { /* noop: no se debe tumbar la app por un IO transitorio */ }
    }

    public void SaveSettings()
    {
        try
        {
            AppPaths.EnsureBase();
            File.WriteAllText(AppPaths.SettingsFile, JsonSerializer.Serialize(Settings, Opts));
        }
        catch { /* noop */ }
    }

    public DownloadEntry? Find(string id) => Entries.Find(e => e.Id == id);
}
