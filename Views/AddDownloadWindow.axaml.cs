using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using AriaXtr.Services;

namespace AriaXtr.Views;

public partial class AddDownloadWindow : Window
{
    public List<string> ResultUrls { get; private set; } = new();
    public string ResultDir { get; private set; } = "";
    public string? ResultFileName { get; private set; }
    public int ResultConnections { get; private set; } = 16;

    public AddDownloadWindow() : this(AppPaths.BaseDir, 16) { }

    public AddDownloadWindow(string defaultDir, int defaultConnections)
    {
        InitializeComponent();
        DirBox.Text = defaultDir;
        ConnSlider.Value = Settings.ClampConnections(defaultConnections);
        ConnText.Text = ((int)ConnSlider.Value).ToString();
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

    private async void Download_Click(object? sender, RoutedEventArgs e) => await TryAcceptAsync();

    private async void UrlsBox_KeyDown(object? sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter && e.KeyModifiers.HasFlag(KeyModifiers.Control))
            await TryAcceptAsync();
    }

    private async Task TryAcceptAsync()
    {
        var urls = (UrlsBox.Text ?? "")
            .Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Where(u => u.Length > 0)
            .Distinct()
            .ToList();

        var dir = (DirBox.Text ?? "").Trim();

        ErrorText.IsVisible = true;
        if (urls.Count == 0)
        {
            ErrorText.Text = Loc.IsEnglish
                ? "Add at least one URL." : "Introduce al menos una URL.";
            return;
        }
        var bad = urls.FirstOrDefault(u =>
            !u.StartsWith("http://", StringComparison.OrdinalIgnoreCase) &&
            !u.StartsWith("https://", StringComparison.OrdinalIgnoreCase) &&
            !u.StartsWith("ftp://", StringComparison.OrdinalIgnoreCase));
        if (bad != null)
        {
            ErrorText.Text = (Loc.IsEnglish ? "Invalid URL: " : "URL no válida: ") + bad;
            return;
        }
        if (dir.Length == 0)
        {
            ErrorText.Text = Loc.IsEnglish
                ? "Choose a destination folder." : "Elige una carpeta de destino.";
            return;
        }
        try
        {
            System.IO.Directory.CreateDirectory(dir);
        }
        catch (Exception ex)
        {
            ErrorText.Text = (Loc.IsEnglish ? "Cannot use folder: " : "No se puede usar la carpeta: ")
                             + ex.Message;
            return;
        }

        ResultUrls = urls;
        ResultDir = dir;
        var fn = (NameBox.Text ?? "").Trim();
        ResultFileName = fn.Length > 0 ? fn : null;
        ResultConnections = (int)ConnSlider.Value;

        Close(true);
        await Task.CompletedTask;
    }

    private void Cancel_Click(object? sender, RoutedEventArgs e) => Close(false);
}
