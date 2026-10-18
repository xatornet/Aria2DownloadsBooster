using Avalonia;

namespace AriaXtr;

internal static class Program
{
    [System.STAThread]
    public static void Main(string[] args)
    {
        // No usar await aquí: lifetime clásico de escritorio.
        BuildAvaloniaApp().StartWithClassicDesktopLifetime(args);
    }

    public static AppBuilder BuildAvaloniaApp()
        => AppBuilder.Configure<App>()
            .UsePlatformDetect()
            .WithInterFont()
            .LogToTrace();
}
