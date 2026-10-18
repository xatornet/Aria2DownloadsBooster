using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using AriaXtr.ViewModels;

namespace AriaXtr.Views;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        try
        {
            Icon = new WindowIcon(AssetLoader.Open(
                new Uri("avares://AriaXtrDownloader/Assets/icon.ico")));
        }
        catch (Exception ex) { Services.UiLog.Write("icono ventana no disponible: " + ex.Message); }
        Closed += (_, _) => (DataContext as MainWindowViewModel)?.ShutdownEngine();
    }

    private void ToggleCompleted_Click(object? sender, RoutedEventArgs e)
    {
        var show = CompletedList.IsVisible;
        CompletedList.IsVisible = !show;
        CompletedChevron.Text = show ? "▶" : "▼";
    }
}
