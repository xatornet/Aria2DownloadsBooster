using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using AriaXtr.Services;
using AriaXtr.ViewModels;
using AriaXtr.Views;

namespace AriaXtr;

public partial class App : Application
{
    public override void Initialize()
    {
        AvaloniaXamlLoader.Load(this);
    }

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            var vm = new MainWindowViewModel();
            var main = new MainWindow { DataContext = vm };
            desktop.MainWindow = main;
            desktop.ShutdownRequested += (_, _) => vm.ShutdownEngine();

            // Cerrar → minimizar a la bandeja (si está activado): el motor sigue descargando.
            main.Closing += (_, e) =>
            {
                if (Tray.ReallyQuit || !vm.AppSettings.MinimizeToTray) return;
                e.Cancel = true;
                main.Hide();
                if (vm.ShouldShowTrayHint())
                    Toast.Show(Loc.TrayHintTitle, Loc.TrayHintBody, Tray.OpenMainWindow);
            };

            Tray.SetEnabled(vm.AppSettings.MinimizeToTray);
        }
        base.OnFrameworkInitializationCompleted();
    }
}
