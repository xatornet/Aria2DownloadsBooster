using System.Collections.Generic;
using System.Globalization;

namespace AriaXtr.Services;

/// <summary>Localización mínima: español por defecto, inglés si el sistema está en inglés.</summary>
public static class Loc
{
    public static bool IsEnglish { get; } =
        CultureInfo.CurrentUICulture.TwoLetterISOLanguageName.ToLowerInvariant() == "en";

    private static string T(string key) =>
        (IsEnglish ? _en : _es).TryGetValue(key, out var v) ? v : key;

    public static string AppTitle => T("app_title");
    public static string NewDownload => T("new_download");
    public static string SettingsBtn => T("settings_btn");
    public static string DownloadsLabel => T("downloads_label");
    public static string CompletedLabel => T("completed_label");
    public static string CleanupCompleted => T("cleanup_completed");
    public static string EngineMissingText => T("engine_missing_text");
    public static string InstallEngine => T("install_engine");
    public static string InstallingEngine => T("installing_engine");
    public static string EngineStarting => T("engine_starting");
    public static string EngineReady => T("engine_ready");
    public static string EngineError => T("engine_error");
    public static string Pause => T("pause");
    public static string Resume => T("resume");
    public static string Retry => T("retry");
    public static string Delete => T("delete");
    public static string OpenFolder => T("open_folder");
    public static string ConnectionsLabel => T("connections_label");
    public static string GlobalSpeed => T("global_speed");
    public static string NoDownloads => T("no_downloads");

    // Diálogo añadir
    public static string AddTitle => T("add_title");
    public static string UrlsLabel => T("urls_label");
    public static string UrlsWatermark => T("urls_watermark");
    public static string FolderLabel => T("folder_label");
    public static string Browse => T("browse");
    public static string FileNameLabel => T("filename_label");
    public static string FileNameWatermark => T("filename_watermark");
    public static string StartNow => T("start_now");
    public static string Cancel => T("cancel");

    // Diálogo ajustes
    public static string SettingsTitle => T("settings_title");
    public static string MaxConcurrent => T("max_concurrent");
    public static string DefaultConnections => T("default_connections");
    public static string DefaultFolder => T("default_folder");
    public static string Save => T("save");

    // Diálogo borrar
    public static string DeleteTitle => T("delete_title");
    public static string DeleteQuestion => T("delete_question");
    public static string DeleteAlsoFile => T("delete_also_file");

    // Estados
    public static string StatusText(string key) => key switch
    {
        "active" => IsEnglish ? "Downloading" : "Descargando",
        "waiting" => IsEnglish ? "Waiting" : "En cola",
        "paused" => IsEnglish ? "Paused" : "Pausada",
        "stopped" => IsEnglish ? "Stopped" : "Detenida",
        "error" => IsEnglish ? "Error" : "Error",
        "complete" => IsEnglish ? "Completed" : "Completada",
        _ => key,
    };

    public static string ThreadsWord => T("threads_word");
    public static string Retrying => T("retrying");
    public static string Details => T("details");
    public static string UrlsCaption => T("urls_caption");
    public static string FolderCaption => T("folder_caption");
    public static string AddedCaption => T("added_caption");

    public static string FormatSpeed(double bytesPerSec) =>
        T("speed") + " " + FormatSize(bytesPerSec) + "/s";

    public static string FormatSize(double bytes)
    {
        string[] units = { "B", "KiB", "MiB", "GiB", "TiB" };
        int i = 0;
        while (bytes >= 1024 && i < units.Length - 1) { bytes /= 1024; i++; }
        return $"{bytes:0.#} {units[i]}";
    }

    private static readonly Dictionary<string, string> _es = new()
    {
        ["app_title"] = "Aria-XTRDownloader",
        ["new_download"] = "+ Nueva descarga",
        ["settings_btn"] = "Ajustes",
        ["downloads_label"] = "Descargas",
        ["completed_label"] = "Completadas",
        ["cleanup_completed"] = "Vaciar completadas",
        ["engine_missing_text"] = "Falta el motor aria2 para este sistema. Descárgalo para empezar.",
        ["install_engine"] = "Descargar motor aria2",
        ["installing_engine"] = "Descargando motor aria2…",
        ["engine_starting"] = "Iniciando motor aria2…",
        ["engine_ready"] = "Motor aria2 listo",
        ["engine_error"] = "Error del motor aria2",
        ["pause"] = "Pausar",
        ["resume"] = "Reanudar",
        ["retry"] = "Reintentar",
        ["delete"] = "Eliminar",
        ["open_folder"] = "Abrir carpeta",
        ["connections_label"] = "Conexiones (hilos)",
        ["global_speed"] = "Velocidad total",
        ["no_downloads"] = "No hay descargas activas.\nPulsa «+ Nueva descarga» para empezar.",
        ["add_title"] = "Nueva descarga",
        ["urls_label"] = "Enlace(s) de descarga — una URL por línea (varias URLs = espejos del mismo archivo)",
        ["urls_watermark"] = "https://ejemplo.com/archivo.zip",
        ["folder_label"] = "Guardar en",
        ["browse"] = "Examinar…",
        ["filename_label"] = "Nombre de archivo (opcional)",
        ["filename_watermark"] = "Automático desde la URL",
        ["start_now"] = "Descargar",
        ["cancel"] = "Cancelar",
        ["settings_title"] = "Ajustes",
        ["max_concurrent"] = "Descargas simultáneas",
        ["default_connections"] = "Conexiones por descarga (máx. 16 hilos)",
        ["default_folder"] = "Carpeta por defecto",
        ["save"] = "Guardar",
        ["delete_title"] = "Confirmar eliminación",
        ["delete_question"] = "¿Eliminar esta descarga de la lista?",
        ["delete_also_file"] = "Borrar también el archivo descargado/parcial",
        ["speed"] = "",
        ["threads_word"] = "hilos",
        ["retrying"] = "Reintentando…",
        ["details"] = "detalles",
        ["urls_caption"] = "URLs:",
        ["folder_caption"] = "Carpeta:",
        ["added_caption"] = "Añadida:",
    };

    private static readonly Dictionary<string, string> _en = new()
    {
        ["app_title"] = "Aria-XTRDownloader",
        ["new_download"] = "+ New download",
        ["settings_btn"] = "Settings",
        ["downloads_label"] = "Downloads",
        ["completed_label"] = "Completed",
        ["cleanup_completed"] = "Clear completed",
        ["engine_missing_text"] = "aria2 engine missing for this system. Download it to get started.",
        ["install_engine"] = "Download aria2 engine",
        ["installing_engine"] = "Downloading aria2 engine…",
        ["engine_starting"] = "Starting aria2 engine…",
        ["engine_ready"] = "aria2 engine ready",
        ["engine_error"] = "aria2 engine error",
        ["pause"] = "Pause",
        ["resume"] = "Resume",
        ["retry"] = "Retry",
        ["delete"] = "Delete",
        ["open_folder"] = "Open folder",
        ["connections_label"] = "Connections (threads)",
        ["global_speed"] = "Total speed",
        ["no_downloads"] = "No active downloads.\nPress «+ New download» to start one.",
        ["add_title"] = "New download",
        ["urls_label"] = "Download URL(s) — one per line (multiple URLs = mirrors of the same file)",
        ["urls_watermark"] = "https://example.com/file.zip",
        ["folder_label"] = "Save to",
        ["browse"] = "Browse…",
        ["filename_label"] = "File name (optional)",
        ["filename_watermark"] = "Automatic from URL",
        ["start_now"] = "Download",
        ["cancel"] = "Cancel",
        ["settings_title"] = "Settings",
        ["max_concurrent"] = "Simultaneous downloads",
        ["default_connections"] = "Connections per download (max 16 threads)",
        ["default_folder"] = "Default folder",
        ["save"] = "Save",
        ["delete_title"] = "Confirm delete",
        ["delete_question"] = "Remove this download from the list?",
        ["delete_also_file"] = "Also delete downloaded/partial file",
        ["speed"] = "",
        ["threads_word"] = "threads",
        ["retrying"] = "Retrying…",
        ["details"] = "details",
        ["urls_caption"] = "URLs:",
        ["folder_caption"] = "Folder:",
        ["added_caption"] = "Added:",
    };
}
