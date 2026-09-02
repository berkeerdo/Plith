using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;
using System.Windows.Threading;
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
        //
        // Delay 2.5 s: Loaded fires BEFORE WPF's first render pass. Without the delay,
        // LaunchPlithAndExit called Application.Shutdown before FinishPage ever painted,
        // so the installer window "just disappeared" after the Registering step — users
        // read the disappearance as a crash. The delay guarantees the page is visible
        // and gives the child Process.Start time to complete before the parent tears down.
        if (vm.OpenAfterInstall)
        {
            Loaded += (_, _) =>
            {
                var timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(2500) };
                timer.Tick += (_, _) =>
                {
                    timer.Stop();
                    LaunchPlithAndExit(installedExePath);
                };
                timer.Start();
            };
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
