using System;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using AriaXtr.Services;

namespace AriaXtr.Views;

public partial class SettingsWindow : Window
{
    public int ResultMaxConcurrent => (int)(MaxConcurrentBox.Value ?? 3);
    public int ResultConnections => (int)ConnSlider.Value;
    public string ResultDefaultDir => (DirBox.Text ?? "").Trim();
    public bool ResultMinimizeToTray => TrayBox.IsChecked == true;

    public SettingsWindow() : this(new Settings()) { }

    public SettingsWindow(Settings s)
    {
        InitializeComponent();
        MaxConcurrentBox.Value = Math.Clamp(s.MaxConcurrentDownloads, 1, 20);
        ConnSlider.Value = Settings.ClampConnections(s.DefaultConnections);
        ConnText.Text = ((int)ConnSlider.Value).ToString();
        DirBox.Text = s.DefaultDir;
        TrayBox.IsChecked = s.MinimizeToTray;
    }

    private async void Browse_Click(object? sender, RoutedEventArgs e)
    {
        var sp = TopLevel.GetTopLevel(this)?.StorageProvider;
        if (sp == null) return;
        var folders = await sp.OpenFolderPickerAsync(new FolderPickerOpenOptions
        {
            Title = Loc.Browse,
            AllowMultiple = false,
        });
        if (folders.Count > 0)
            DirBox.Text = folders[0].Path.LocalPath;
    }

    private void ConnSlider_OnValueChanged(object? sender, RangeBaseValueChangedEventArgs e)
    {
        if (ConnText != null)
            ConnText.Text = ((int)ConnSlider.Value).ToString();
    }

    private void Save_Click(object? sender, RoutedEventArgs e)
    {
        if (ResultDefaultDir.Length == 0) return;
        Close(true);
    }

    private void Cancel_Click(object? sender, RoutedEventArgs e) => Close(false);
}
