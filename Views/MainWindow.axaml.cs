using Avalonia.Controls;
using Avalonia.Interactivity;
using AriaXtr.ViewModels;

namespace AriaXtr.Views;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        Closed += (_, _) => (DataContext as MainWindowViewModel)?.ShutdownEngine();
    }

    private void ToggleCompleted_Click(object? sender, RoutedEventArgs e)
    {
        var show = CompletedList.IsVisible;
        CompletedList.IsVisible = !show;
        CompletedChevron.Text = show ? "▶" : "▼";
    }
}
