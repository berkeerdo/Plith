using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;
using Plith.Installer.ViewModels;

namespace Plith.Installer.Pages;

public partial class FinishPage : UserControl
{
    public FinishPage(InstallerViewModel vm, string installedExePath)
    {
        InitializeComponent();
        DataContext = vm;

        SubtitleText.Text = vm.GameModeEnabled
            ? "Game mode is active — OSD draws over fullscreen games."
            : "OSD draws over borderless fullscreen games.";

        OpenPlithButton.Visibility = vm.OpenAfterInstall ? Visibility.Visible : Visibility.Collapsed;

        OpenPlithButton.Click += (_, _) =>
        {
            // Launch via explorer.exe so the new process runs in the user context (not admin).
            // Direct Process.Start from an elevated installer fails for UIAccess binaries
            // ("A referral was returned from the server").
            Process.Start(new ProcessStartInfo("explorer.exe", $"\"{installedExePath}\""));
            Application.Current.Shutdown();
        };

        GitHubButton.Click += (_, _) => Process.Start(new ProcessStartInfo("https://github.com/berkeerdo/Plith") { UseShellExecute = true });
        CloseButton.Click += (_, _) => Application.Current.Shutdown();

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }
}
