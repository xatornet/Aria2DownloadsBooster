using System;
using System.IO;

namespace AriaXtr.Services;

/// <summary>Log mínimo de la UI para diagnosticar flujos (añadir, pausar, errores de sondeo).</summary>
public static class UiLog
{
    private static readonly object Lk = new();
    private static long _lastRotateCheck;

    public static void Write(string msg)
    {
        try
        {
            lock (Lk)
            {
                AppPaths.EnsureBase();
                // rotación simple: si pasa de 512 KB, se reinicia (una vez por minuto como mucho)
                var now = DateTime.Now.Ticks;
                if (now - _lastRotateCheck > TimeSpan.TicksPerMinute)
                {
                    _lastRotateCheck = now;
                    var fi = new FileInfo(AppPaths.UiLogFile);
                    if (fi.Exists && fi.Length > 512 * 1024)
                        File.Delete(AppPaths.UiLogFile);
                }
                File.AppendAllText(AppPaths.UiLogFile,
                    $"{DateTime.Now:HH:mm:ss.fff} {msg}{Environment.NewLine}");
            }
        }
        catch { /* el log nunca debe tumbar la app */ }
    }
}
