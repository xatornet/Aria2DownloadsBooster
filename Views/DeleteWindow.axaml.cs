using Avalonia.Controls;
using Avalonia.Interactivity;

namespace AriaXtr.Views;

public partial class DeleteWindow : Window
{
    public bool DeleteFileRequested => DeleteFileCheck.IsChecked == true;

    public DeleteWindow() : this("?") { }

    public DeleteWindow(string name)
    {
        InitializeComponent();
        NameText.Text = name;
    }

    private void Ok_Click(object? sender, RoutedEventArgs e) => Close(true);
    private void Cancel_Click(object? sender, RoutedEventArgs e) => Close(false);
}
