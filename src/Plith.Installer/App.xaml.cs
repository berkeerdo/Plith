using System.Threading;
using System.Windows;
using Plith.Installer.Pages;
using Plith.Installer.Services;
using Plith.Installer.ViewModels;

namespace Plith.Installer;

#pragma warning disable CA1001
// App holds a disposable Mutex but cannot implement IDisposable — WPF Application's
// lifecycle is owned by the framework. Mutex disposed in OnExit instead. Suppression
// scoped to the type declaration where CA1001 fires.
public partial class App : Application
#pragma warning restore CA1001
{
    private const string SingleInstanceMutexName = "Global\\Plith.Installer.SingleInstance.7F9C8E1A";
    private Mutex? _singleInstanceMutex;

    public bool IsUninstallMode { get; private set; }

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // initiallyOwned: false because we only need the mutex as a system-wide existence flag —
        // checking createdNew is what tells us we're the first instance. Owning the mutex would
        // require ReleaseMutex() from the same thread on shutdown, which throws if this is the
        // second-instance path that never acquired ownership.
        _singleInstanceMutex = new Mutex(initiallyOwned: false, SingleInstanceMutexName, out bool createdNew);
        if (!createdNew)
        {
            MessageBox.Show("Plith Setup is already running.",
                "Plith Setup", MessageBoxButton.OK, MessageBoxImage.Information);
            Shutdown();
            return;
        }

        IsUninstallMode = e.Args.Length > 0 && e.Args[0] == "--uninstall";

        var log = new LogService();
        var vm = new InstallerViewModel();
        var cert = new CertService();
        var shortcut = new ShortcutService(log);
        var registry = new RegistryService();
        var orchestrator = new InstallOrchestrator(log, cert, shortcut, registry, vm);

        var window = new MainWindow();
        window.Show();

        if (IsUninstallMode)
            RouteUninstallFlow(window, vm, orchestrator, log);
        else
            RouteInstallFlow(window, vm, orchestrator, log);
    }

    private static void RouteInstallFlow(MainWindow window, InstallerViewModel vm,
        InstallOrchestrator orchestrator, LogService log)
    {
        var detector = new InstallDetector();
        var existing = detector.GetInstalledVersion();
        vm.NewVersion = typeof(App).Assembly.GetName().Version?.ToString(3) ?? "0.1.0";

        if (existing is null) vm.Mode = InstallerMode.FreshInstall;
        else if (existing == vm.NewVersion) { vm.Mode = InstallerMode.Reinstall; vm.ExistingVersion = existing; }
        else { vm.Mode = InstallerMode.Update; vm.ExistingVersion = existing; }

        var welcome = new WelcomePage(vm);
        welcome.PrimaryClicked += async (_, _) =>
        {
            orchestrator.PrepareSteps();
            window.NavigateTo(new ProgressPage(vm, vm.Mode == InstallerMode.Update
                ? "Updating Plith…"
                : "Installing Plith…"));
            try
            {
                await orchestrator.RunInstallAsync();
                window.NavigateTo(new FinishPage(vm, InstallOrchestrator.InstalledExe));
            }
#pragma warning disable CA1031
            catch
#pragma warning restore CA1031
            {
                window.NavigateTo(new ErrorPage(vm, log));
            }
        };
        window.NavigateTo(welcome);
    }

    private static void RouteUninstallFlow(MainWindow window, InstallerViewModel vm,
        InstallOrchestrator orchestrator, LogService log)
    {
        var confirm = new UninstallConfirmPage();
        confirm.UninstallClicked += async (_, _) =>
        {
            orchestrator.PrepareUninstallSteps();
            window.NavigateTo(new ProgressPage(vm, "Uninstalling Plith…"));
            try
            {
                await orchestrator.RunUninstallAsync();
                window.NavigateTo(new UninstallFinishPage());
            }
#pragma warning disable CA1031
            catch
#pragma warning restore CA1031
            {
                window.NavigateTo(new ErrorPage(vm, log));
            }
        };
        window.NavigateTo(confirm);
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _singleInstanceMutex?.Dispose();
        base.OnExit(e);
    }
}
