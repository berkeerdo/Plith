using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace Plith.Installer.ViewModels;

public enum InstallerMode
{
    FreshInstall,
    Reinstall,
    Update,
}

public sealed class InstallerViewModel : INotifyPropertyChanged
{
    private InstallerMode _mode = InstallerMode.FreshInstall;
    private string? _existingVersion;
    private string _newVersion = "0.1.0";
    private bool _gameModeEnabled = true;
    private bool _autoStartEnabled = true;
    private bool _openAfterInstall = true;
    private double _progress;
    private string _errorMessage = string.Empty;
    private string _failedStepTitle = string.Empty;

    public ObservableCollection<InstallStepViewModel> Steps { get; } = new();

    public InstallerMode Mode
    {
        get => _mode;
        set { _mode = value; OnPropertyChanged(); OnPropertyChanged(nameof(PrimaryButtonLabel)); }
    }

    public string? ExistingVersion
    {
        get => _existingVersion;
        set { _existingVersion = value; OnPropertyChanged(); OnPropertyChanged(nameof(PrimaryButtonLabel)); }
    }

    public string NewVersion
    {
        get => _newVersion;
        set { _newVersion = value; OnPropertyChanged(); OnPropertyChanged(nameof(PrimaryButtonLabel)); }
    }

    public bool GameModeEnabled
    {
        get => _gameModeEnabled;
        set { _gameModeEnabled = value; OnPropertyChanged(); }
    }

    public bool AutoStartEnabled
    {
        get => _autoStartEnabled;
        set { _autoStartEnabled = value; OnPropertyChanged(); }
    }

    public bool OpenAfterInstall
    {
        get => _openAfterInstall;
        set { _openAfterInstall = value; OnPropertyChanged(); }
    }

    public double Progress
    {
        get => _progress;
        set { _progress = value; OnPropertyChanged(); }
    }

    public string ErrorMessage
    {
        get => _errorMessage;
        set { _errorMessage = value; OnPropertyChanged(); }
    }

    public string FailedStepTitle
    {
        get => _failedStepTitle;
        set { _failedStepTitle = value; OnPropertyChanged(); }
    }

    /// <summary>Computed text for the Welcome page primary button.</summary>
    public string PrimaryButtonLabel => Mode switch
    {
        InstallerMode.Reinstall => $"Reinstall Plith v{ExistingVersion}",
        InstallerMode.Update => $"Update Plith v{ExistingVersion} → v{NewVersion}",
        _ => "Install Plith",
    };

    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
