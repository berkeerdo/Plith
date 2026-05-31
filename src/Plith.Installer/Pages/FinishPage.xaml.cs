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

        OpenPlithButton.Click += (_, _) => LaunchPlithAndExit(installedExePath);

        // M2 + I5: also fire the launch automatically if the user opted in. The button
        // remains visible as a fallback affordance while the new process is spinning up.
        if (vm.OpenAfterInstall)
        {
            Loaded += (_, _) => LaunchPlithAndExit(installedExePath);
        }

        GitHubButton.Click += (_, _) => Process.Start(new ProcessStartInfo("https://github.com/berkeerdo/Plith") { UseShellExecute = true });
        CloseButton.Click += (_, _) => Application.Current.Shutdown();

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }

    private static void LaunchPlithAndExit(string installedExePath)
    {
        try
        {
            // Launch via explorer.exe so the new process runs in the user context (not admin).
            // Direct Process.Start from an elevated installer fails for UIAccess binaries
            // ("A referral was returned from the server").
            Process.Start(new ProcessStartInfo("explorer.exe", $"\"{installedExePath}\""));
        }
        catch
        {
            // explorer killed by malware / locked down — fall through to Shutdown so the
            // wizard doesn't hang on an unhandled exception.
        }
        Application.Current.Shutdown();
    }
}
