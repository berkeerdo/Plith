using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;
using Plith.Installer.Services;
using Plith.Installer.ViewModels;

namespace Plith.Installer.Pages;

public partial class ErrorPage : UserControl
{
    public ErrorPage(InstallerViewModel vm, LogService log)
    {
        InitializeComponent();
        DataContext = vm;

        FailedStepText.Text = $"Step: \"{vm.FailedStepTitle}\"";
        ErrorMessageText.Text = vm.ErrorMessage;

        CopyLogButton.Click += (_, _) => Clipboard.SetText(log.ReadAll());
        OpenLogButton.Click += (_, _) => Process.Start(new ProcessStartInfo(log.LogPath) { UseShellExecute = true });
        CloseButton.Click += (_, _) => Application.Current.Shutdown();

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }
}
