using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace Plith.Installer.ViewModels;

public enum InstallStepStatus
{
    Pending,
    Running,
    Done,
    Failed,
}

public sealed class InstallStepViewModel : INotifyPropertyChanged
{
    private string _title = string.Empty;
    private InstallStepStatus _status = InstallStepStatus.Pending;
    private string? _failureMessage;

    public string Title
    {
        get => _title;
        set { if (_title != value) { _title = value; OnPropertyChanged(); } }
    }

    public InstallStepStatus Status
    {
        get => _status;
        set { if (_status != value) { _status = value; OnPropertyChanged(); } }
    }

    public string? FailureMessage
    {
        get => _failureMessage;
        set { if (_failureMessage != value) { _failureMessage = value; OnPropertyChanged(); } }
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
