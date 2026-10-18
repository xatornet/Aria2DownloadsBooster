using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using Avalonia.Threading;

namespace AriaXtr.Services;

/// <summary>Icono de bandeja del sistema (⚡) con menú Abrir/Salir.</summary>
public static class Tray
{
    private static TrayIcon? _icon;

    /// <summary>Verdadero cuando el usuario elige «Salir»: el cierre real no debe cancelarse.</summary>
    public static bool ReallyQuit { get; set; }

    public static void SetEnabled(bool enabled)
    {
        if (Application.Current is not { } app) return;
        if (enabled)
        {
            // icono siempre nuevo: re-adjuntar uno antes quitado puede fallar
            _icon = Build();
            TrayIcon.SetIcons(app, new TrayIcons { _icon });
        }
        else
        {
            TrayIcon.SetIcons(app, new TrayIcons());
        }
    }

    private static TrayIcon Build()
    {
        var openItem = new NativeMenuItem(Loc.TrayOpen);
        openItem.Click += (_, _) => OpenMainWindow();

        var exitItem = new NativeMenuItem(Loc.TrayExit);
        exitItem.Click += (_, _) =>
        {
            ReallyQuit = true;
            if (Application.Current?.ApplicationLifetime is IClassicDesktopStyleApplicationLifetime d)
                d.Shutdown();
        };

        var menu = new NativeMenu();
        menu.Add(openItem);
        menu.Add(new NativeMenuItemSeparator());
        menu.Add(exitItem);

        var icon = new TrayIcon
        {
            Icon = AppIcon(),
            ToolTipText = "Aria-XTRDownloader",
            Menu = menu,
        };
        icon.Clicked += (_, _) => OpenMainWindow();
        return icon;
    }

    public static void OpenMainWindow()
    {
        if (Application.Current?.ApplicationLifetime is not IClassicDesktopStyleApplicationLifetime d
            || d.MainWindow == null) return;
        var w = d.MainWindow;
        w.Show();
        if (w.WindowState == WindowState.Minimized) w.WindowState = WindowState.Normal;
        w.Activate();
    }

    /// <summary>Icono de la app (Assets/icon.ico); si falta, lo dibuja en runtime.</summary>
    private static WindowIcon AppIcon()
    {
        try
        {
            return new WindowIcon(AssetLoader.Open(
                new Uri("avares://AriaXtrDownloader/Assets/icon.ico")));
        }
        catch
        {
            return DrawIcon();
        }
    }

    /// <summary>Dibuja el icono ⚡ en runtime (no hay .ico en el repo): rayo vectorial sobre fondo oscuro.</summary>
    private static WindowIcon DrawIcon()
    {
        var rtb = new RenderTargetBitmap(new PixelSize(64, 64), new Vector(96, 96));
        using (var ctx = rtb.CreateDrawingContext())
        {
            ctx.DrawGeometry(
                new SolidColorBrush(Color.FromRgb(0x1E, 0x22, 0x35)), null,
                new RectangleGeometry { Rect = new Rect(1, 1, 62, 62), RadiusX = 14, RadiusY = 14 });

            var figure = new PathFigure { StartPoint = new Point(40, 4), IsClosed = true };
            foreach (var p in new[]
            {
                new Point(18, 36), new Point(29, 36),
                new Point(25, 58), new Point(48, 26), new Point(34, 26),
            })
                figure.Segments!.Add(new LineSegment { Point = p });
            var geo = new PathGeometry();
            geo.Figures!.Add(figure);
            ctx.DrawGeometry(new SolidColorBrush(Color.FromRgb(0x60, 0xA5, 0xFA)), null, geo);
        }
        using var ms = new MemoryStream();
        rtb.Save(ms);
        ms.Position = 0;
        return new WindowIcon(ms);
    }
}

/// <summary>Mini-notificación tipo toast: abajo a la derecha, auto-cierre, clic = abrir la app.</summary>
public static class Toast
{
    private const int W = 360, H = 92;
    private static readonly List<Window> Alive = new();

    public static void Show(string title, string body, Action? onClick = null)
    {
        var main = (Application.Current?.ApplicationLifetime as IClassicDesktopStyleApplicationLifetime)?.MainWindow;
        var screen = main != null ? (main.Screens.Primary ?? main.Screens.All.FirstOrDefault()) : null;
        if (screen == null) return;

        var scale = screen.Scaling;
        var idx = Math.Min(Alive.Count, 3); // apila hacia arriba, máx. 4
        var x = (int)(screen.WorkingArea.Right - (W + 14) * scale);
        var y = (int)(screen.WorkingArea.Bottom - (H + 14) * scale - idx * (int)((H + 10) * scale));

        var win = new Window
        {
            Width = W,
            Height = H,
            SystemDecorations = SystemDecorations.None,
            ShowInTaskbar = false,
            ShowActivated = false,
            Topmost = true,
            CanResize = false,
            Background = Brushes.Transparent,
            TransparencyLevelHint = new[] { WindowTransparencyLevel.Transparent },
            Position = new PixelPoint(Math.Max(x, screen.WorkingArea.X), Math.Max(y, screen.WorkingArea.Y)),
            Content = new Border
            {
                CornerRadius = new CornerRadius(10),
                Background = new SolidColorBrush(Color.Parse("#1F2437")),
                BorderBrush = new SolidColorBrush(Color.Parse("#3B82F6")),
                BorderThickness = new Thickness(1),
                Padding = new Thickness(14, 10, 14, 10),
                Child = new StackPanel
                {
                    Spacing = 4,
                    Children =
                    {
                        new TextBlock
                        {
                            Text = "⚡ " + title,
                            Foreground = new SolidColorBrush(Color.Parse("#7AA2FF")),
                            FontWeight = FontWeight.SemiBold,
                        },
                        new TextBlock
                        {
                            Text = body,
                            TextWrapping = TextWrapping.Wrap,
                            Foreground = new SolidColorBrush(Color.Parse("#D6DAF0")),
                        },
                    },
                },
            },
        };

        if (onClick != null)
            win.PointerPressed += (_, _) => { onClick(); win.Close(); };

        Alive.Add(win);
        win.Closed += (_, _) => Alive.Remove(win);

        var timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(6.5) };
        timer.Tick += (_, _) => { timer.Stop(); win.Close(); };
        timer.Start();

        win.Show();
    }
}
